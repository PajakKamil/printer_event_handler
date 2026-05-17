//! Printer monitoring API.
//!
//! [`PrinterMonitor`] is the entry point - cheaply cloneable, wraps an
//! `Arc<dyn PrinterBackend>` so multiple tasks share a single backend
//! connection. The change-detection methods are split across private
//! submodules (single-printer flows vs. the concurrent multi-printer
//! variant) for readability; every `impl` hangs off this same
//! [`PrinterMonitor`] type, so the public surface is unchanged.

use std::sync::Arc;

use crate::Result;
use crate::backend::{PrinterBackend, create_backend};
use crate::logging::pe_info as info;

mod builder;
mod presence;
mod property;
mod single;
mod summary;
// `multi` re-uses `presence::PresenceTracker`; it's declared `pub(super)`
// inside that module so this `mod multi` (sibling) can name it.
mod multi;

pub use builder::MonitorBuilder;
pub use property::MonitorableProperty;
pub use summary::PrinterSummary;

#[cfg(test)]
mod observation_tests;

/// How many consecutive transient backend errors a monitor loop tolerates
/// before propagating the failure to the caller. A successful poll resets the
/// counter, so this only fires for sustained outages (e.g. WMI service down,
/// CUPS unreachable) - a single WMI hiccup no longer kills monitoring.
pub(super) const MAX_CONSECUTIVE_MONITOR_ERRORS: u32 = 5;

/// Printer monitoring and querying functionality.
///
/// `PrinterMonitor` is cheaply cloneable (uses `Arc` internally) and can be
/// shared across tasks without creating multiple backend connections.
#[derive(Clone)]
pub struct PrinterMonitor {
    pub(super) backend: Arc<dyn PrinterBackend>,
}

impl PrinterMonitor {
    /// Creates a new PrinterMonitor instance with the appropriate platform backend.
    ///
    /// This function automatically selects and initializes the correct backend
    /// for the current platform (WMI for Windows, CUPS for Linux).
    ///
    /// # Returns
    /// * `Result<Self>` - A new PrinterMonitor instance or an error if initialization fails
    ///
    /// # Errors
    /// * `PrinterError::PlatformNotSupported` - If the current platform is not supported
    /// * `PrinterError::WmiError` - If WMI initialization fails on Windows
    /// * `PrinterError::CupsError` - If CUPS initialization fails on Linux
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    /// }
    /// ```
    pub async fn new() -> Result<Self> {
        info!("Initializing printer monitor...");
        let backend = create_backend().await?;
        Ok(Self {
            backend: Arc::from(backend),
        })
    }

    /// Constructs a monitor around a caller-supplied backend.
    ///
    /// Intended for tests and advanced users who need to substitute the
    /// platform backend (e.g. recording, fault injection, scripted
    /// responses for end-to-end change-detection tests). Production code
    /// should use [`PrinterMonitor::new`] instead - it auto-selects the
    /// real WMI/CUPS backend for the current platform.
    ///
    /// # Example
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use printer_event_handler::PrinterMonitor;
    /// use printer_event_handler::backend::PrinterBackend;
    ///
    /// fn make_monitor(backend: Arc<dyn PrinterBackend>) -> PrinterMonitor {
    ///     PrinterMonitor::from_backend(backend)
    /// }
    /// ```
    pub fn from_backend(backend: Arc<dyn PrinterBackend>) -> Self {
        Self { backend }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(windows)]
    async fn test_monitor_creation() {
        let result = PrinterMonitor::new().await;
        // This might fail in CI/test environments without proper WMI access
        // but it should at least compile and attempt the connection
        match result {
            Ok(_) => println!("Monitor created successfully"),
            Err(e) => println!("Expected error in test environment: {}", e),
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_monitor_unix_creation() {
        let result = PrinterMonitor::new().await;
        // On Unix/Linux, the monitor should be created successfully
        assert!(result.is_ok());
    }

    #[test]
    fn test_printer_monitor_is_clone() {
        // Test that PrinterMonitor implements Clone
        fn assert_clone<T: Clone>() {}
        assert_clone::<PrinterMonitor>();
    }
}
