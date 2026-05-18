//! `Stream`-flavoured terminal methods for [`MonitorBuilder`].
//!
//! These mirror [`MonitorBuilder::run_changes`] / [`MonitorBuilder::run_property`]
//! but return a [`tokio_stream::Stream`] instead of invoking a callback.
//! Internally, a background task runs the existing callback-driven monitor
//! loop and forwards each event through an unbounded
//! [`tokio::sync::mpsc::channel`]. The stream completes when the underlying
//! monitor exits (cancellation, sustained backend failure, or receiver drop).
//!
//! Unbounded is appropriate because change events are rare (poll cadence is
//! seconds-to-minutes, not microseconds); the cost of an unbounded queue is
//! negligible and we avoid the blocking-vs-drop tradeoff a bounded channel
//! would introduce.

use tokio_stream::Stream;
use tokio_stream::wrappers::UnboundedReceiverStream;

use super::builder::MonitorBuilder;
use crate::logging::pe_warn as warn;
use crate::{PrinterChanges, PropertyChange, Result};

impl<'a> MonitorBuilder<'a> {
    /// Stream variant of [`MonitorBuilder::run_changes`].
    ///
    /// Returns a [`Stream`] yielding `Result<PrinterChanges>` as the
    /// monitor detects changes. Successful diffs arrive as `Ok(...)`; a
    /// terminal `Err(...)` is emitted exactly once when the monitor exits
    /// because of sustained backend failure (more than
    /// `MAX_CONSECUTIVE_MONITOR_ERRORS` consecutive WMI/CUPS errors), or
    /// the `wait_for_appearance=false` pre-check failed. The stream then
    /// closes. A clean shutdown (cancellation, receiver drop) closes the
    /// stream without emitting an error.
    ///
    /// The returned future drives the monitor in a background task spawned
    /// onto the current tokio runtime; the caller doesn't need to spawn
    /// anything themselves.
    ///
    /// [`CancellationToken`]: tokio_util::sync::CancellationToken
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    /// use tokio_stream::StreamExt;
    ///
    /// # async fn _docs() {
    /// let monitor = PrinterMonitor::new().await.unwrap();
    /// let mut stream = monitor.monitor("HP LaserJet").run_changes_stream();
    /// while let Some(item) = stream.next().await {
    ///     match item {
    ///         Ok(changes) => println!("changes: {}", changes.summary()),
    ///         Err(e) => eprintln!("monitor stopped: {}", e),
    ///     }
    /// }
    /// # }
    /// ```
    pub fn run_changes_stream(self) -> impl Stream<Item = Result<PrinterChanges>> + Send + 'static {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<PrinterChanges>>();

        let monitor = self.monitor.clone();
        let printer_name = self.printer_name;
        let interval_ms = self.interval_ms;
        let cancel_token = self.cancel_token;
        let wait_for_appearance = self.wait_for_appearance;
        let use_events = self.use_events;

        tokio::spawn(async move {
            // Pre-check is applied uniformly to BOTH the polling and the
            // event-driven paths. Before this was only applied to polling,
            // so `with_events(true).wait_for_appearance(false)` would
            // happily subscribe to events for a printer that doesn't exist
            // and leave the stream silent forever.
            if !wait_for_appearance && !pre_check_present(&monitor, &printer_name, &tx).await {
                return;
            }

            // When the `events` feature is on AND the caller opted in via
            // `with_events(true)`, route through the platform-specific
            // event subscription. Otherwise fall through to polling. The
            // _ = use_events suppresses the unused-variable warning on
            // targets where neither cfg branch consumes it.
            #[cfg(all(windows, feature = "events"))]
            if use_events {
                let (event_tx, mut event_rx) =
                    tokio::sync::mpsc::unbounded_channel::<PrinterChanges>();
                super::events::spawn_event_subscription(
                    event_tx,
                    printer_name.clone(),
                    cancel_token,
                );
                while let Some(changes) = event_rx.recv().await {
                    if tx.send(Ok(changes)).is_err() {
                        return;
                    }
                }
                return;
            }
            #[cfg(all(unix, feature = "events"))]
            if use_events {
                let (event_tx, mut event_rx) =
                    tokio::sync::mpsc::unbounded_channel::<PrinterChanges>();
                super::events_cups::spawn_cups_subscription(
                    event_tx,
                    printer_name.clone(),
                    cancel_token,
                    monitor.clone(),
                );
                while let Some(changes) = event_rx.recv().await {
                    if tx.send(Ok(changes)).is_err() {
                        return;
                    }
                }
                return;
            }
            #[cfg(not(any(all(windows, feature = "events"), all(unix, feature = "events"))))]
            if use_events {
                warn!(
                    "MonitorBuilder::with_events(true) requires the `events` cargo feature; falling back to polling"
                );
            }
            let _ = use_events;

            let tx_cb = tx.clone();
            let result = monitor
                .monitor_printer_changes(&printer_name, interval_ms, cancel_token, move |changes| {
                    // Receiver drop is the normal way callers terminate the
                    // stream; quietly swallow the SendError.
                    let _ = tx_cb.send(Ok(changes.clone()));
                })
                .await;
            // Forward sustained backend failure as the stream's final
            // item; callers can otherwise not distinguish "cancelled
            // gracefully" from "WMI/CUPS service died".
            if let Err(e) = result {
                let _ = tx.send(Err(e));
            }
        });

        UnboundedReceiverStream::new(rx)
    }

    /// Stream variant of [`MonitorBuilder::run_property`].
    ///
    /// Requires [`MonitorBuilder::filter_property`] to have been called -
    /// returns `Err(PrinterError::Other(...))` otherwise. Each emitted
    /// [`PropertyChange`] is the filtered property; other property mutations
    /// in the same poll are dropped (the callback equivalent has the same
    /// semantics).
    pub fn run_property_stream(
        self,
    ) -> Result<impl Stream<Item = Result<PropertyChange>> + Send + 'static> {
        let property = self.property_filter.clone().ok_or_else(|| {
            crate::PrinterError::Other(
                "MonitorBuilder::run_property_stream requires filter_property(...) to be set"
                    .to_string(),
            )
        })?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<PropertyChange>>();
        let monitor = self.monitor.clone();
        let printer_name = self.printer_name;
        let interval_ms = self.interval_ms;
        let cancel_token = self.cancel_token;
        let wait_for_appearance = self.wait_for_appearance;

        tokio::spawn(async move {
            if !wait_for_appearance && !pre_check_present(&monitor, &printer_name, &tx).await {
                return;
            }

            let tx_cb = tx.clone();
            let result = monitor
                .monitor_printer_changes(&printer_name, interval_ms, cancel_token, move |changes| {
                    for change in &changes.changes {
                        if change.property() == property {
                            let _ = tx_cb.send(Ok(change.clone()));
                        }
                    }
                })
                .await;
            if let Err(e) = result {
                let _ = tx.send(Err(e));
            }
        });

        Ok(UnboundedReceiverStream::new(rx))
    }
}

/// Shared pre-check used by both stream terminal methods when
/// `wait_for_appearance` is `false`. Returns `true` if the caller should
/// continue (printer exists), `false` if the stream should end. On the
/// "should end" path, a typed error is forwarded through `tx` so callers
/// reading the stream can distinguish "printer missing at startup" from
/// "backend query failed during pre-check" from "cancelled".
async fn pre_check_present<T>(
    monitor: &super::PrinterMonitor,
    printer_name: &str,
    tx: &tokio::sync::mpsc::UnboundedSender<Result<T>>,
) -> bool {
    match monitor.backend.find_printer(printer_name).await {
        Ok(Some(_)) => true,
        Ok(None) => {
            warn!(
                "Printer '{}' not present and wait_for_appearance=false; stream ending",
                printer_name
            );
            let _ = tx.send(Err(crate::PrinterError::PrinterNotFound(
                printer_name.to_string(),
            )));
            false
        }
        Err(e) => {
            warn!(
                "Failed pre-check for printer '{}': {}; stream ending",
                printer_name, e
            );
            let _ = tx.send(Err(e));
            false
        }
    }
}
