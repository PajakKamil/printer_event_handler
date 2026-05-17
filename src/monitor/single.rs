use std::collections::HashMap;

use crate::logging::{pe_error as error, pe_info as info, pe_warn as warn};
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;

use crate::{Printer, PrinterChanges, Result};

use super::property::MonitorableProperty;
use super::summary::PrinterSummary;
use super::{MAX_CONSECUTIVE_MONITOR_ERRORS, PrinterMonitor};

impl PrinterMonitor {
    /// Retrieves a list of all printers available on the system.
    ///
    /// This method queries the platform-specific printer service to get
    /// information about all installed and available printers.
    ///
    /// # Returns
    /// * `Result<Vec<Printer>>` - A vector of all printers found on the system
    ///
    /// # Errors
    /// * `PrinterError::WmiError` - If the WMI query fails on Windows
    /// * `PrinterError::CupsError` - If the CUPS query fails on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let printers = monitor.list_printers().await.unwrap();
    ///
    ///     for printer in printers {
    ///         println!("{}: {}", printer.name(), printer.status_description());
    ///     }
    /// }
    /// ```
    #[deprecated(
        since = "1.5.0",
        note = "use `list_printers_cancellable` to support cancellation; this method will be removed in 2.0"
    )]
    pub async fn list_printers(&self) -> Result<Vec<Printer>> {
        self.backend.list_printers().await
    }

    /// Cancellable variant of [`Self::list_printers`].
    ///
    /// When `cancel_token` is provided, the in-flight backend query is raced
    /// against `token.cancelled()` and the call returns
    /// [`PrinterError::Cancelled`] as soon as the token fires. Passing `None`
    /// is equivalent to the deprecated [`Self::list_printers`].
    ///
    /// Note that the backend query (a WMI call on Windows, an `lpstat` exec on
    /// Linux) is not itself abortable - cancellation surfaces as soon as the
    /// current poll completes, not mid-flight.
    pub async fn list_printers_cancellable(
        &self,
        cancel_token: Option<CancellationToken>,
    ) -> Result<Vec<Printer>> {
        match cancel_token {
            Some(token) => tokio::select! {
                result = self.backend.list_printers() => result,
                _ = token.cancelled() => Err(crate::PrinterError::Cancelled),
            },
            None => self.backend.list_printers().await,
        }
    }

    /// Searches for a specific printer by name using case-insensitive matching.
    ///
    /// This method searches through all available printers to find one with
    /// a name that matches the provided string (case-insensitive).
    ///
    /// # Arguments
    /// * `name` - The name of the printer to search for
    ///
    /// # Returns
    /// * `Result<Option<Printer>>` - The found printer or None if not found
    ///
    /// # Errors
    /// * `PrinterError::WmiError` - If the WMI query fails on Windows
    /// * `PrinterError::CupsError` - If the CUPS query fails on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///
    ///     if let Some(printer) = monitor.find_printer("HP LaserJet").await.unwrap() {
    ///         println!("Found printer: {}", printer.name());
    ///     }
    /// }
    /// ```
    #[deprecated(
        since = "1.5.0",
        note = "use `find_printer_cancellable` to support cancellation; this method will be removed in 2.0"
    )]
    pub async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        self.backend.find_printer(name).await
    }

    /// Cancellable variant of [`Self::find_printer`].
    ///
    /// See [`Self::list_printers_cancellable`] for the cancellation semantics
    /// (token races against the in-flight backend query and yields
    /// [`PrinterError::Cancelled`]).
    pub async fn find_printer_cancellable(
        &self,
        name: &str,
        cancel_token: Option<CancellationToken>,
    ) -> Result<Option<Printer>> {
        match cancel_token {
            Some(token) => tokio::select! {
                result = self.backend.find_printer(name) => result,
                _ = token.cancelled() => Err(crate::PrinterError::Cancelled),
            },
            None => self.backend.find_printer(name).await,
        }
    }

    /// Continuously monitors a specific printer for status changes.
    ///
    /// This function runs indefinitely, polling the specified printer every `interval_ms`
    /// milliseconds and calling the provided callback function whenever the printer's status changes.
    /// The callback receives both the current printer state and the previous state (if any).
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `callback` - Function called when printer status changes, receives (current, previous)
    ///
    /// # Returns
    /// * `Result<()>` - Never returns Ok normally (runs indefinitely), only Err on failure
    ///
    /// # Errors
    /// * `PrinterError::PrinterNotFound` - If the specified printer is not found initially
    /// * `PrinterError::WmiError` - If WMI queries fail on Windows
    /// * `PrinterError::CupsError` - If CUPS queries fail on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Behavior
    /// - If the printer disappears during monitoring, the callback is called with a synthetic
    ///   "unknown" status to indicate the printer is no longer available
    /// - The first check always triggers the callback to provide the initial status
    /// - Subsequent calls only trigger the callback if the status actually changes
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     monitor.monitor_printer("HP LaserJet", 30000, Some(cancel_token.clone()), |current, previous| {
    ///         if let Some(prev) = previous {
    ///             if prev != current {
    ///                 println!("Status changed: {} -> {}",
    ///                     prev.status_description(),
    ///                     current.status_description());
    ///             }
    ///         } else {
    ///             println!("Initial status: {}", current.status_description());
    ///         }
    ///     }).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_printer<F>(
        &self,
        printer_name: &str,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&Printer, Option<&Printer>) + Send,
    {
        info!("Starting printer monitoring service for: {}", printer_name);

        let mut previous_printer: Option<Printer> = None;
        let mut consecutive_errors: u32 = 0;

        loop {
            // Check for cancellation
            if let Some(ref token) = cancel_token
                && token.is_cancelled()
            {
                info!("Printer monitoring for '{}' cancelled", printer_name);
                return Ok(());
            }

            match self.backend.find_printer(printer_name).await {
                Ok(Some(current_printer)) => {
                    consecutive_errors = 0;
                    info!("Checking printer: {}", current_printer.name());
                    let has_changed = previous_printer
                        .as_ref()
                        .map(|prev| prev != &current_printer)
                        .unwrap_or(true);

                    if has_changed {
                        callback(&current_printer, previous_printer.as_ref());
                        info!(
                            "Printer '{}' - Status: {}, Error: {}",
                            printer_name,
                            current_printer.status_description(),
                            current_printer.error_description()
                        );
                        previous_printer = Some(current_printer);
                    } else {
                        info!("Printer '{}' status unchanged", printer_name);
                    }
                }
                Ok(None) => {
                    consecutive_errors = 0;
                    warn!("Printer '{}' not found", printer_name);
                    if previous_printer.is_some() {
                        // Printer was previously found but now missing
                        callback(
                            &Printer::new(
                                printer_name.to_string(),
                                crate::PrinterStatus::StatusUnknown,
                                crate::ErrorState::UnknownError,
                                true,
                                false,
                            ),
                            previous_printer.as_ref(),
                        );
                        previous_printer = None;
                    }
                }
                Err(e) => {
                    consecutive_errors += 1;
                    if consecutive_errors >= MAX_CONSECUTIVE_MONITOR_ERRORS {
                        error!(
                            "Printer '{}' monitoring failed after {} consecutive errors, last: {}",
                            printer_name, consecutive_errors, e
                        );
                        return Err(e);
                    }
                    warn!(
                        "Transient error checking printer '{}' (attempt {}/{}), will retry: {}",
                        printer_name, consecutive_errors, MAX_CONSECUTIVE_MONITOR_ERRORS, e
                    );
                }
            }

            // Sleep with cancellation support
            if let Some(ref token) = cancel_token {
                tokio::select! {
                    _ = sleep(Duration::from_millis(interval_ms)) => {},
                    _ = token.cancelled() => {
                        info!("Printer monitoring for '{}' cancelled during sleep", printer_name);
                        return Ok(());
                    }
                }
            } else {
                sleep(Duration::from_millis(interval_ms)).await;
            }
        }
    }

    /// Retrieves a comprehensive summary of all printers and their current states.
    ///
    /// This method provides a convenient way to get an overview of all printers
    /// in a structured format, useful for status dashboards or reports.
    ///
    /// # Returns
    /// * `Result<HashMap<String, PrinterSummary>>` - Map of printer names to their summaries
    ///
    /// # Errors
    /// * `PrinterError::WmiError` - If the WMI query fails on Windows
    /// * `PrinterError::CupsError` - If the CUPS query fails on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let summary = monitor.printer_summary().await.unwrap();
    ///
    ///     for (name, info) in summary {
    ///         println!("{}: {} ({})", name, info.status,
    ///             if info.has_error { "ERROR" } else { "OK" });
    ///     }
    /// }
    /// ```
    pub async fn printer_summary(&self) -> Result<HashMap<String, PrinterSummary>> {
        let printers = self.backend.list_printers().await?;
        let mut summary = HashMap::new();

        for printer in printers {
            summary.insert(
                printer.name().to_string(),
                PrinterSummary {
                    status: printer.status().clone(),
                    error_state: printer.error_state().clone(),
                    is_offline: printer.is_offline(),
                    is_default: printer.is_default(),
                    has_error: printer.has_error(),
                },
            );
        }

        Ok(summary)
    }

    /// Iterator counterpart to [`Self::printer_summary`].
    ///
    /// Returns the same `(name, summary)` pairs without allocating a
    /// [`HashMap`]. Prefer this when you just need to iterate once - for
    /// example to log every printer or feed a UI list - and don't need
    /// key-based lookup.
    ///
    /// Order matches the underlying backend's enumeration order (insertion
    /// order via the source `Vec<Printer>`), not the unspecified order a
    /// `HashMap` iterator would yield.
    ///
    /// # Errors
    /// Same errors as [`Self::printer_summary`].
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     for (name, info) in monitor.printer_summary_iter().await.unwrap() {
    ///         println!("{name}: {} ({})", info.status,
    ///             if info.has_error { "ERROR" } else { "OK" });
    ///     }
    /// }
    /// ```
    pub async fn printer_summary_iter(
        &self,
    ) -> Result<impl Iterator<Item = (String, PrinterSummary)>> {
        let printers = self.backend.list_printers().await?;
        let pairs: Vec<(String, PrinterSummary)> = printers
            .into_iter()
            .map(|printer| {
                let summary = PrinterSummary {
                    status: printer.status().clone(),
                    error_state: printer.error_state().clone(),
                    is_offline: printer.is_offline(),
                    is_default: printer.is_default(),
                    has_error: printer.has_error(),
                };
                (printer.name().to_string(), summary)
            })
            .collect();
        Ok(pairs.into_iter())
    }

    /// Monitors a printer with detailed property change detection.
    ///
    /// This enhanced monitoring method provides detailed information about exactly which
    /// properties changed between checks, enabling fine-grained monitoring and alerting.
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `cancel_token` - Optional cancellation token for graceful shutdown
    /// * `callback` - Function called when properties change, receives PrinterChanges
    ///
    /// # Returns
    /// * `Result<()>` - Returns Ok when cancelled, or Err on failure
    ///
    /// # Behavior
    /// - The callback is **NOT** called on the initial state capture
    /// - Only actual property changes trigger the callback
    /// - Changes are always non-empty when the callback is invoked
    /// - The initial state is captured silently for comparison with future states
    /// - Disappearance fires a synthetic `IsOffline: false -> true` change
    /// - Reappearance fires a change set comparing the missing-state snapshot
    ///   with the current printer (typically includes `IsOffline: true -> false`)
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
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     // Clone the token for the monitoring task
    ///     let token_clone = cancel_token.clone();
    ///     let handle = tokio::spawn(async move {
    ///         monitor.monitor_printer_changes("HP LaserJet", 30000, Some(token_clone), |changes| {
    ///             println!("Detected {} changes:", changes.change_count());
    ///             for change in &changes.changes {
    ///                 println!("  - {}", change.description());
    ///             }
    ///         }).await
    ///     });
    ///
    ///     // Later: cancel monitoring
    ///     cancel_token.cancel();
    ///     handle.await.unwrap().unwrap();
    /// }
    /// ```
    pub async fn monitor_printer_changes<F>(
        &self,
        printer_name: &str,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&PrinterChanges) + Send,
    {
        info!(
            "Starting detailed printer change monitoring for: {}",
            printer_name
        );

        // `previous_printer` is the snapshot the next poll compares against.
        // After a disappearance it's replaced with a synthetic "missing"
        // snapshot so reappearance surfaces the IsOffline:true->false delta and
        // any other properties that changed during the gap (B4).
        let mut previous_printer: Option<Printer> = None;
        let mut was_present: bool = false;
        let mut consecutive_errors: u32 = 0;

        loop {
            // Check for cancellation
            if let Some(ref token) = cancel_token
                && token.is_cancelled()
            {
                info!("Printer monitoring for '{}' cancelled", printer_name);
                return Ok(());
            }

            match self.backend.find_printer(printer_name).await {
                Ok(Some(current_printer)) => {
                    consecutive_errors = 0;
                    if let Some(ref prev) = previous_printer {
                        let changes = prev.compare_with(&current_printer);
                        if changes.has_changes() {
                            info!(
                                "Printer '{}' - {} properties changed",
                                printer_name,
                                changes.change_count()
                            );
                            callback(&changes);
                        }
                    } else {
                        // Initial state - just capture, don't call callback
                        info!("Printer '{}' - Initial state captured", printer_name);
                    }
                    previous_printer = Some(current_printer);
                    was_present = true;
                }
                Ok(None) => {
                    consecutive_errors = 0;
                    warn!("Printer '{}' not found", printer_name);
                    if was_present {
                        if let Some(ref prev) = previous_printer {
                            // Fresh disappearance - synthesize an IsOffline change.
                            let mut changes = PrinterChanges::new(printer_name.to_string());
                            changes.changes.push(crate::PropertyChange::IsOffline {
                                old: prev.is_offline(),
                                new: true,
                            });
                            callback(&changes);
                        }
                        // Replace with a synthetic "missing" snapshot so the
                        // next successful poll surfaces the reappearance.
                        previous_printer = Some(Printer::new(
                            printer_name.to_string(),
                            crate::PrinterStatus::Offline,
                            crate::ErrorState::UnknownError,
                            true,
                            false,
                        ));
                    }
                    was_present = false;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    if consecutive_errors >= MAX_CONSECUTIVE_MONITOR_ERRORS {
                        error!(
                            "Printer '{}' monitoring failed after {} consecutive errors, last: {}",
                            printer_name, consecutive_errors, e
                        );
                        return Err(e);
                    }
                    warn!(
                        "Transient error checking printer '{}' (attempt {}/{}), will retry: {}",
                        printer_name, consecutive_errors, MAX_CONSECUTIVE_MONITOR_ERRORS, e
                    );
                }
            }

            // Sleep with cancellation support
            if let Some(ref token) = cancel_token {
                tokio::select! {
                    _ = sleep(Duration::from_millis(interval_ms)) => {},
                    _ = token.cancelled() => {
                        info!("Printer monitoring for '{}' cancelled during sleep", printer_name);
                        return Ok(());
                    }
                }
            } else {
                sleep(Duration::from_millis(interval_ms)).await;
            }
        }
    }

    /// Monitors a specific property of a printer for changes.
    ///
    /// This method allows monitoring just a single property, useful for alerting
    /// on specific conditions like offline status or error state changes.
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `property` - The specific property to watch using MonitorableProperty enum
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `cancel_token` - Optional cancellation token for graceful shutdown
    /// * `callback` - Function called when the property changes
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::{PrinterMonitor, MonitorableProperty};
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     monitor.monitor_property(
    ///         "HP LaserJet",
    ///         MonitorableProperty::IsOffline,
    ///         60000,
    ///         Some(cancel_token.clone()),
    ///         |change| {
    ///             println!("Offline status changed: {}", change.description());
    ///         }
    ///     ).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_property<F>(
        &self,
        printer_name: &str,
        property: MonitorableProperty,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&crate::PropertyChange) + Send,
    {
        let property_name = property.as_str();
        info!(
            "Starting property '{}' monitoring for printer: {}",
            property_name, printer_name
        );

        self.monitor_printer_changes(printer_name, interval_ms, cancel_token, move |changes| {
            for change in &changes.changes {
                if change.property_name() == property_name {
                    callback(change);
                }
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use tokio::time::timeout;

    #[tokio::test]
    #[cfg(windows)]
    #[allow(deprecated)]
    async fn test_list_printers_windows() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let printers = monitor.list_printers().await;
            // Either we get printers or an error, but it should return something
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers", printer_list.len());
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    #[allow(deprecated)]
    async fn test_list_printers_unix() {
        let monitor = PrinterMonitor::new().await;
        assert!(monitor.is_ok());

        if let Ok(monitor) = monitor {
            let printers = monitor.list_printers().await;
            // Should return either printers or an error, but not panic
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers", printer_list.len());
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_find_nonexistent_printer() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let result = monitor.find_printer("NonExistentPrinter_12345_ABCDE").await;
            match result {
                Ok(None) => {
                    // Expected: printer not found
                }
                Ok(Some(_)) => {
                    panic!("Unexpectedly found a printer with unlikely name");
                }
                Err(_) => {
                    // Also acceptable in test environments
                }
            }
        }
    }

    #[tokio::test]
    async fn test_printer_summary() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let summary = monitor.printer_summary().await;
            // Should return either a summary or an error, but not panic
            match summary {
                Ok(summary_map) => {
                    println!("Got summary for {} printers", summary_map.len());
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_printer_summary_iter_matches_map() {
        // The iterator variant must yield the same (name, summary) pairs as the
        // HashMap-returning variant - same set of names, equal summaries.
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let (map_result, iter_result) =
                tokio::join!(monitor.printer_summary(), monitor.printer_summary_iter());
            if let (Ok(summary_map), Ok(iter)) = (map_result, iter_result) {
                let iter_vec: Vec<(String, _)> = iter.collect();
                assert_eq!(iter_vec.len(), summary_map.len());
                for (name, summary) in iter_vec {
                    let from_map = summary_map.get(&name).expect("name must be in map");
                    assert_eq!(from_map.status, summary.status);
                    assert_eq!(from_map.error_state, summary.error_state);
                    assert_eq!(from_map.is_offline, summary.is_offline);
                    assert_eq!(from_map.is_default, summary.is_default);
                    assert_eq!(from_map.has_error, summary.has_error);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_cancellation_token() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            // Spawn a task that will be cancelled
            let handle = tokio::spawn(async move {
                monitor
                    .monitor_printer_changes(
                        "NonExistentPrinter_Test",
                        1000,
                        Some(cancel_clone),
                        |_changes| {
                            // This should never be called
                            panic!("Should not receive changes for nonexistent printer");
                        },
                    )
                    .await
            });

            // Wait a bit then cancel
            tokio::time::sleep(StdDuration::from_millis(100)).await;
            cancel_token.cancel();

            // The task should complete quickly after cancellation
            let result = timeout(StdDuration::from_secs(2), handle).await;
            assert!(result.is_ok(), "Task should complete after cancellation");
            if let Ok(Ok(task_result)) = result {
                assert!(task_result.is_ok(), "Cancelled monitoring should return Ok");
            }
        }
    }

    #[tokio::test]
    async fn test_monitor_without_cancellation() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            // Monitor without a cancellation token should still work
            let handle = tokio::spawn(async move {
                monitor
                    .monitor_printer_changes(
                        "NonExistentPrinter_Test",
                        5000,
                        None, // No cancellation token
                        |_changes| {
                            // This should never be called
                        },
                    )
                    .await
            });

            // Let it run a bit
            tokio::time::sleep(StdDuration::from_millis(100)).await;

            // Abort the task since we have no cancellation token
            handle.abort();

            // Task should be aborted
            let result = handle.await;
            assert!(result.is_err(), "Task should be aborted");
        }
    }
}
