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
    /// Returns a [`Stream`] yielding each [`PrinterChanges`] as the monitor
    /// detects them. The stream ends when the monitor loop exits (either
    /// because the [`CancellationToken`] was cancelled, the backend failed
    /// past the consecutive-error tolerance, or the receiver was dropped).
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
    /// while let Some(changes) = stream.next().await {
    ///     println!("changes: {}", changes.summary());
    /// }
    /// # }
    /// ```
    pub fn run_changes_stream(self) -> impl Stream<Item = PrinterChanges> + Send + 'static {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PrinterChanges>();

        // When the `events` feature is on AND the caller opted in via
        // `with_events(true)` AND we're on Windows, route through the WMI
        // notification subscription. Otherwise fall through to polling.
        #[cfg(all(windows, feature = "events"))]
        if self.use_events {
            super::events::spawn_event_subscription(
                tx,
                self.printer_name,
                self.cancel_token,
            );
            return UnboundedReceiverStream::new(rx);
        }
        #[cfg(not(all(windows, feature = "events")))]
        if self.use_events {
            warn!(
                "MonitorBuilder::with_events(true) requires the `events` cargo feature on Windows; falling back to polling"
            );
        }

        let monitor = self.monitor.clone();
        let printer_name = self.printer_name;
        let interval_ms = self.interval_ms;
        let cancel_token = self.cancel_token;
        let wait_for_appearance = self.wait_for_appearance;

        tokio::spawn(async move {
            if !wait_for_appearance {
                match monitor.backend.find_printer(&printer_name).await {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        warn!(
                            "Printer '{}' not present and wait_for_appearance=false; stream ending",
                            printer_name
                        );
                        return;
                    }
                    Err(e) => {
                        warn!(
                            "Failed pre-check for printer '{}': {}; stream ending",
                            printer_name, e
                        );
                        return;
                    }
                }
            }

            let tx_cb = tx.clone();
            let _ = monitor
                .monitor_printer_changes(&printer_name, interval_ms, cancel_token, move |changes| {
                    // Receiver drop is the normal way callers terminate the
                    // stream; quietly swallow the SendError.
                    let _ = tx_cb.send(changes.clone());
                })
                .await;
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
    ) -> Result<impl Stream<Item = PropertyChange> + Send + 'static> {
        let property = self.property_filter.clone().ok_or_else(|| {
            crate::PrinterError::Other(
                "MonitorBuilder::run_property_stream requires filter_property(...) to be set"
                    .to_string(),
            )
        })?;
        let property_name = property.as_str().to_string();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PropertyChange>();
        let monitor = self.monitor.clone();
        let printer_name = self.printer_name;
        let interval_ms = self.interval_ms;
        let cancel_token = self.cancel_token;
        let wait_for_appearance = self.wait_for_appearance;

        tokio::spawn(async move {
            if !wait_for_appearance {
                match monitor.backend.find_printer(&printer_name).await {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => return,
                }
            }

            let tx_cb = tx.clone();
            let _ = monitor
                .monitor_printer_changes(&printer_name, interval_ms, cancel_token, move |changes| {
                    for change in &changes.changes {
                        if change.property_name() == property_name {
                            let _ = tx_cb.send(change.clone());
                        }
                    }
                })
                .await;
        });

        Ok(UnboundedReceiverStream::new(rx))
    }
}
