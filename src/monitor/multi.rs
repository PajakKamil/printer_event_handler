use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::logging::{pe_error as error, pe_info as info, pe_warn as warn};
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;

use crate::{Printer, PrinterChanges, Result};

use super::presence::PresenceTracker;
use super::{MAX_CONSECUTIVE_MONITOR_ERRORS, PrinterMonitor};

impl PrinterMonitor {
    /// Monitors multiple printers concurrently and reports changes for any of them.
    ///
    /// This method allows monitoring several printers simultaneously, with a single
    /// callback that receives changes from any of the monitored printers.
    ///
    /// # Arguments
    /// * `printer_names` - List of printer names to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `cancel_token` - Optional cancellation token for graceful shutdown
    /// * `callback` - Function called when any printer changes (NOT called on initial state)
    ///
    /// # Returns
    /// * `Result<()>` - Returns Ok when cancelled or all tasks complete, or Err on failure
    ///
    /// # Behavior
    /// - The callback is **NOT** called on the initial state capture for each printer
    /// - Only actual property changes trigger the callback
    /// - Disappearance fires a synthetic `IsOffline: false -> true` change once per gap
    /// - Reappearance fires a change set comparing the missing-state snapshot
    ///   with the current printer (typically includes `IsOffline: true -> false`)
    /// - Each printer is monitored in a separate task for true concurrent monitoring
    /// - Callbacks are called outside of internal locks to prevent contention with slow callbacks
    /// - The monitor is cloned (cheaply via Arc) for each task, sharing the same backend connection
    /// - Monitoring stops gracefully when the cancellation token is cancelled
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let printers = vec!["HP LaserJet".to_string(), "Canon Printer".to_string()];
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     let token_clone = cancel_token.clone();
    ///     let handle = tokio::spawn(async move {
    ///         monitor.monitor_multiple_printers(printers, 30000, Some(token_clone), |changes| {
    ///             println!("Printer '{}' changed: {}", changes.printer_name, changes.summary());
    ///         }).await
    ///     });
    ///
    ///     // Later: cancel monitoring
    ///     cancel_token.cancel();
    ///     handle.await.unwrap().unwrap();
    /// }
    /// ```
    pub async fn monitor_multiple_printers<F>(
        &self,
        printer_names: Vec<String>,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(&PrinterChanges) + Send + Sync + 'static,
    {
        // Dedupe before spawning. Two tasks polling the same printer would
        // race on a shared `PresenceTracker` and flip-flop the "initial
        // state captured" vs "synthetic IsOffline" outputs based on which
        // task won each scheduler tick.
        let mut seen: HashSet<String> = HashSet::with_capacity(printer_names.len());
        let printer_names: Vec<String> = printer_names
            .into_iter()
            .filter(|name| {
                let inserted = seen.insert(name.clone());
                if !inserted {
                    warn!(
                        "monitor_multiple_printers: duplicate printer name '{}' ignored",
                        name
                    );
                }
                inserted
            })
            .collect();

        info!(
            "Starting concurrent monitoring of {} printers",
            printer_names.len()
        );

        let callback = Arc::new(callback);
        // JoinSet (over Vec<JoinHandle>) so a single task failure can abort the
        // siblings via `abort_all()`, preventing the orphaned-poller leak that
        // happened when the caller passed no CancellationToken.
        let mut tasks: JoinSet<Result<()>> = JoinSet::new();
        // task-id → printer-name map. `JoinError::id()` carries the same id
        // returned by `JoinSet::spawn`, so when a task panics we can correlate
        // back to which printer's task failed and surface that in
        // `PrinterError::TaskPanicked.printer_name` (F6).
        let mut task_owner: HashMap<tokio::task::Id, String> = HashMap::new();

        for printer_name in printer_names {
            let callback_clone = callback.clone();
            let printer_name_clone = printer_name.clone();
            let monitor_clone = self.clone(); // Cheap Arc clone - shares the same backend
            let cancel_token_clone = cancel_token.clone();

            // Each task owns its own `PresenceTracker` outright. The previous
            // implementation routed all trackers through a single
            // `Arc<Mutex<HashMap<String, PresenceTracker>>>` even though the
            // tasks' keys partition disjointly - the mutex serialised tasks
            // for no correctness benefit. Local ownership removes the
            // contention and also removes the callback-under-lock concern
            // entirely.
            let abort_handle = tasks.spawn(async move {
                let mut tracker = PresenceTracker::new();
                let mut consecutive_errors: u32 = 0;

                loop {
                    // Check for cancellation
                    if let Some(ref token) = cancel_token_clone
                        && token.is_cancelled()
                    {
                        info!("Printer monitoring for '{}' cancelled", printer_name_clone);
                        return Ok(());
                    }

                    match monitor_clone.backend.find_printer(&printer_name_clone).await {
                        Ok(Some(current_printer)) => {
                            consecutive_errors = 0;
                            let changes_to_report = if let Some(ref prev) = tracker.snapshot {
                                let changes = prev.compare_with(&current_printer);
                                if changes.has_changes() {
                                    Some(changes)
                                } else {
                                    None
                                }
                            } else {
                                info!(
                                    "Printer '{}' - Initial state captured",
                                    printer_name_clone
                                );
                                None
                            };

                            tracker.snapshot = Some(current_printer);
                            tracker.was_present_last_poll = true;

                            if let Some(changes) = changes_to_report {
                                info!(
                                    "Printer '{}' - {} properties changed",
                                    printer_name_clone,
                                    changes.change_count()
                                );
                                callback_clone(&changes);
                            }
                        }
                        Ok(None) => {
                            consecutive_errors = 0;
                            warn!("Printer '{}' not found", printer_name_clone);

                            if tracker.was_present_last_poll {
                                // Fresh disappearance - synthesize IsOffline transition.
                                if let Some(ref prev) = tracker.snapshot {
                                    let mut changes =
                                        PrinterChanges::new(printer_name_clone.clone());
                                    changes.changes.push(crate::PropertyChange::IsOffline {
                                        old: prev.is_offline(),
                                        new: true,
                                    });
                                    callback_clone(&changes);
                                }
                                // Replace the snapshot with a synthetic "missing"
                                // baseline so the next successful poll surfaces
                                // the reappearance delta (B4).
                                tracker.snapshot = Some(Printer::new(
                                    printer_name_clone.clone(),
                                    crate::PrinterStatus::Offline,
                                    crate::ErrorState::UnknownError,
                                    true,
                                    false,
                                ));
                            }
                            tracker.was_present_last_poll = false;
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            if consecutive_errors >= MAX_CONSECUTIVE_MONITOR_ERRORS {
                                error!(
                                    "Printer '{}' monitoring failed after {} consecutive errors, last: {}",
                                    printer_name_clone, consecutive_errors, e
                                );
                                return Err(e);
                            }
                            warn!(
                                "Transient error checking printer '{}' (attempt {}/{}), will retry: {}",
                                printer_name_clone, consecutive_errors, MAX_CONSECUTIVE_MONITOR_ERRORS, e
                            );
                        }
                    }

                    // Sleep with cancellation support
                    if let Some(ref token) = cancel_token_clone {
                        tokio::select! {
                            _ = sleep(Duration::from_millis(interval_ms)) => {},
                            _ = token.cancelled() => {
                                info!("Printer monitoring for '{}' cancelled during sleep", printer_name_clone);
                                return Ok(());
                            }
                        }
                    } else {
                        sleep(Duration::from_millis(interval_ms)).await;
                    }
                }
            });
            task_owner.insert(abort_handle.id(), printer_name);
        }

        // Process tasks as they finish; on the first real error, abort siblings
        // and return. JoinError::is_cancelled() is expected after abort_all() so
        // those drain silently.
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {
                    info!("Monitoring task completed successfully");
                }
                Ok(Err(e)) => {
                    error!("Monitoring task failed: {}", e);
                    tasks.abort_all();
                    return Err(e);
                }
                Err(je) if je.is_cancelled() => {
                    // Sibling aborted via abort_all() above - expected, ignore.
                }
                Err(je) => {
                    let task_id = je.id();
                    let owner = task_owner.remove(&task_id);
                    tasks.abort_all();
                    if je.is_panic() {
                        // Pull the payload string out of the panic so the
                        // caller can see what actually went wrong. F6 lifts
                        // this from `PrinterError::Other(format!(...))` into
                        // a typed `TaskPanicked` variant so callers can match
                        // on it without string-parsing.
                        let panic_message = match je.try_into_panic() {
                            Ok(payload) => payload
                                .downcast_ref::<&'static str>()
                                .map(|s| (*s).to_string())
                                .or_else(|| payload.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "non-string panic payload".to_string()),
                            Err(je) => je.to_string(),
                        };
                        let err = crate::PrinterError::TaskPanicked {
                            printer_name: owner,
                            panic_message,
                        };
                        error!("{}", err);
                        return Err(err);
                    }
                    // Non-panic join failure (e.g. runtime shutdown) stays
                    // on `Other` - it isn't a printer-task panic.
                    let detail = format!("monitoring task join failed: {}", je);
                    error!("{}", detail);
                    return Err(crate::PrinterError::Other(detail));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_multiple_printers_cancellation() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            let printer_names = vec!["Printer1".to_string(), "Printer2".to_string()];

            // Spawn monitoring task
            let handle = tokio::spawn(async move {
                monitor
                    .monitor_multiple_printers(
                        printer_names,
                        1000,
                        Some(cancel_clone),
                        |_changes| {
                            // Should not be called
                        },
                    )
                    .await
            });

            // Wait a bit then cancel
            tokio::time::sleep(StdDuration::from_millis(100)).await;
            cancel_token.cancel();

            // Should complete quickly
            let result = timeout(StdDuration::from_secs(2), handle).await;
            assert!(
                result.is_ok(),
                "Multi-printer monitoring should complete after cancellation"
            );
        }
    }
}
