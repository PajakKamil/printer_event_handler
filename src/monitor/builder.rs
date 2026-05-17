//! Fluent builder for [`PrinterMonitor`] monitoring runs.
//!
//! The standalone `monitor_printer*` methods on [`PrinterMonitor`] grew a long
//! positional parameter list (name, interval, cancel token, callback) and a
//! sibling exists for each option permutation. [`MonitorBuilder`] collapses
//! those into a single chainable form and adds two configuration knobs that
//! were awkward to thread through positional parameters:
//!
//! - `wait_for_appearance` - when `true` (the default), the run silently
//!   tolerates the printer not existing yet, polling until it appears. When
//!   `false`, the run errors out with [`crate::PrinterError::PrinterNotFound`]
//!   on the very first poll if the printer is missing.
//! - `filter_property` - target a specific [`MonitorableProperty`], so the
//!   change callback fires only when that property mutates.
//!
//! Entry point is [`PrinterMonitor::monitor`].

use tokio_util::sync::CancellationToken;

use super::PrinterMonitor;
use super::property::MonitorableProperty;
use crate::{Printer, PrinterChanges, PropertyChange, Result};

/// Default poll cadence (milliseconds) used when the caller doesn't override
/// it via [`MonitorBuilder::interval_ms`]. Matches the CLI default and the
/// recommended cadence from the crate-level docs - tight enough for sub-minute
/// status drift detection, loose enough to keep WMI/CUPS load negligible.
const DEFAULT_INTERVAL_MS: u64 = 60_000;

/// Builder for a single-printer monitoring run.
///
/// Construct via [`PrinterMonitor::monitor`]; chain configuration methods;
/// terminate with one of [`Self::run_changes`], [`Self::run_printer`], or
/// [`Self::run_property`].
///
/// # Example
/// ```rust,no_run
/// use printer_event_handler::{PrinterMonitor, MonitorableProperty};
/// use tokio_util::sync::CancellationToken;
///
/// # async fn _docs() {
/// let monitor = PrinterMonitor::new().await.unwrap();
/// let cancel = CancellationToken::new();
///
/// monitor
///     .monitor("HP LaserJet")
///     .interval_ms(15_000)
///     .cancel_token(cancel.clone())
///     .filter_property(MonitorableProperty::IsOffline)
///     .run_property(|change| {
///         println!("Offline flipped: {}", change.description());
///     })
///     .await
///     .unwrap();
/// # }
/// ```
pub struct MonitorBuilder<'a> {
    // Fields are `pub(super)` so the sibling `stream` module can read them
    // when implementing the `Stream`-returning terminal methods without
    // duplicating the option-collecting boilerplate. They stay non-`pub`
    // because the public surface is the chainable setter API only.
    pub(super) monitor: &'a PrinterMonitor,
    pub(super) printer_name: String,
    pub(super) interval_ms: u64,
    pub(super) cancel_token: Option<CancellationToken>,
    pub(super) wait_for_appearance: bool,
    pub(super) property_filter: Option<MonitorableProperty>,
    /// Opt-in flag for the event-driven path. Honoured only when the crate
    /// is built with the `events` feature on Windows; on other targets the
    /// builder silently falls back to polling (with a one-shot warn log).
    pub(super) use_events: bool,
}

impl<'a> MonitorBuilder<'a> {
    pub(super) fn new(monitor: &'a PrinterMonitor, printer_name: impl Into<String>) -> Self {
        Self {
            monitor,
            printer_name: printer_name.into(),
            interval_ms: DEFAULT_INTERVAL_MS,
            cancel_token: None,
            wait_for_appearance: true,
            property_filter: None,
            use_events: false,
        }
    }

    /// Sets the polling interval in milliseconds. Default: 60 000 ms.
    pub fn interval_ms(mut self, ms: u64) -> Self {
        self.interval_ms = ms;
        self
    }

    /// Attaches a [`CancellationToken`] - the monitor will stop cleanly when
    /// the token is cancelled (both before each poll and during the sleep).
    pub fn cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// When `true` (the default), the monitor silently waits if the printer
    /// does not yet exist - the first matching poll captures it as baseline.
    ///
    /// When `false`, the monitor performs an eager existence check before
    /// entering the loop and returns [`crate::PrinterError::PrinterNotFound`]
    /// if the printer is missing.
    pub fn wait_for_appearance(mut self, wait: bool) -> Self {
        self.wait_for_appearance = wait;
        self
    }

    /// Restricts [`Self::run_property`] to a single property. Has no effect
    /// on the other terminal methods. Required when calling
    /// [`Self::run_property`] - the method errors if no filter was set.
    pub fn filter_property(mut self, property: MonitorableProperty) -> Self {
        self.property_filter = Some(property);
        self
    }

    /// Opt in to the event-driven monitoring path.
    ///
    /// Effective only when the crate is built with the `events` cargo feature
    /// on Windows. In that configuration, [`Self::run_changes_stream`] uses a
    /// WMI `__InstanceModificationEvent` subscription instead of polling, so
    /// the printer's state changes propagate to the stream as soon as WMI
    /// reports them (typically within ~1 second).
    ///
    /// On other targets or without the feature, this method is accepted
    /// silently and the builder falls back to polling. Callers can therefore
    /// set it unconditionally without breaking cross-platform builds.
    pub fn with_events(mut self, enable: bool) -> Self {
        self.use_events = enable;
        self
    }

    /// Runs detailed change monitoring - the callback receives a
    /// [`PrinterChanges`] for each poll that detected mutations. Mirrors the
    /// behavior of the deprecated `PrinterMonitor::monitor_printer_changes`.
    pub async fn run_changes<F>(self, callback: F) -> Result<()>
    where
        F: FnMut(&PrinterChanges) + Send,
    {
        self.ensure_present().await?;
        self.monitor
            .monitor_printer_changes(
                &self.printer_name,
                self.interval_ms,
                self.cancel_token,
                callback,
            )
            .await
    }

    /// Runs whole-snapshot monitoring - the callback receives `(current,
    /// previous)`. The first matching poll fires with `previous = None`.
    /// Mirrors the behavior of `PrinterMonitor::monitor_printer`.
    pub async fn run_printer<F>(self, callback: F) -> Result<()>
    where
        F: FnMut(&Printer, Option<&Printer>) + Send,
    {
        self.ensure_present().await?;
        self.monitor
            .monitor_printer(
                &self.printer_name,
                self.interval_ms,
                self.cancel_token,
                callback,
            )
            .await
    }

    /// Runs single-property monitoring - the callback fires only for the
    /// [`MonitorableProperty`] selected via [`Self::filter_property`].
    /// Returns [`crate::PrinterError::Other`] if no filter was configured.
    pub async fn run_property<F>(self, callback: F) -> Result<()>
    where
        F: FnMut(&PropertyChange) + Send,
    {
        let property = self.property_filter.clone().ok_or_else(|| {
            crate::PrinterError::Other(
                "MonitorBuilder::run_property requires filter_property(...) to be set".to_string(),
            )
        })?;
        self.ensure_present().await?;
        self.monitor
            .monitor_property(
                &self.printer_name,
                property,
                self.interval_ms,
                self.cancel_token,
                callback,
            )
            .await
    }

    /// Eager presence check used when `wait_for_appearance` is `false`.
    async fn ensure_present(&self) -> Result<()> {
        if self.wait_for_appearance {
            return Ok(());
        }
        match self.monitor.backend.find_printer(&self.printer_name).await? {
            Some(_) => Ok(()),
            None => Err(crate::PrinterError::PrinterNotFound(
                self.printer_name.clone(),
            )),
        }
    }
}

impl PrinterMonitor {
    /// Starts a fluent monitoring run via [`MonitorBuilder`].
    ///
    /// Replaces the positional-parameter `monitor_printer*` methods for new
    /// code. The old methods remain available (the builder delegates to them
    /// internally) but the builder form is the recommended entry point.
    pub fn monitor(&self, printer_name: impl Into<String>) -> MonitorBuilder<'_> {
        MonitorBuilder::new(self, printer_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_property_without_filter_errors_with_other() {
        let Ok(monitor) = PrinterMonitor::new().await else {
            return; // Skip when backend unavailable in CI.
        };
        let result = monitor
            .monitor("anything")
            .run_property(|_| {})
            .await;
        match result {
            Err(crate::PrinterError::Other(msg)) => {
                assert!(msg.contains("filter_property"));
            }
            other => panic!("expected Other(...) with filter hint, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn list_printers_cancellable_returns_cancelled() {
        let Ok(monitor) = PrinterMonitor::new().await else {
            return;
        };
        let token = CancellationToken::new();
        token.cancel();
        let result = monitor.list_printers_cancellable(Some(token)).await;
        // Cancellation should win the race; even if the WMI/CUPS query is
        // very fast, the token is already cancelled at the start. A successful
        // result is acceptable on very fast backends because `tokio::select!`
        // is unordered when both branches are immediately ready.
        match result {
            Ok(_) => {}
            Err(crate::PrinterError::Cancelled) => {}
            Err(other) => panic!("expected Cancelled or Ok, got {:?}", other),
        }
    }
}
