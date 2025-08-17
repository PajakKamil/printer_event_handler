//! # Printer Event Handler
//!
//! A cross-platform printer status monitoring library for Windows and Linux systems.
//! This library provides functionality to query printer status, monitor printer events, and
//! track printer state changes using platform-specific backends:
//! - **Windows**: WMI (Windows Management Instrumentation) with **complete Win32_Printer support**
//!   - All 26 PrinterState values (0-25): Idle, Paused, Error, PaperJam, Busy, etc.
//!   - All 12 DetectedErrorState values (0-11): NoError, NoPaper, Jammed, ServiceRequested, etc.
//! - **Linux**: CUPS (Common Unix Printing System) with basic status detection
//!
//! ## Features
//!
//! - **Comprehensive Windows support** - Full Win32_Printer coverage per Microsoft documentation
//! - **Cross-platform support** (Windows and Linux)
//! - **Real-time monitoring** - Query all printers on the system
//! - **Status change detection** - Monitor specific printers for status changes
//! - **Async/await support** with Tokio
//! - **Detailed status information** - 26+ printer statuses and 11 error states
//! - **Platform-specific backends** with unified API
//!
//! ## Example
//!
//! ```rust,no_run
//! use printer_event_handler::{PrinterMonitor, PrinterError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), PrinterError> {
//!     let monitor = PrinterMonitor::new().await?;
//!     
//!     // List all printers
//!     let printers = monitor.list_printers().await?;
//!     for printer in printers {
//!         println!("Printer: {}", printer.name());
//!         println!("Status: {}", printer.status_description());
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod error;
pub mod monitor;
pub mod printer;

pub use error::PrinterError;
pub use monitor::PrinterMonitor;
pub use printer::{ErrorState, Printer, PrinterStatus};

/// Result type used throughout the library
pub type Result<T> = std::result::Result<T, PrinterError>;

#[cfg(test)]
mod tests {
    #[test]
    fn test_library_compiles() {
        // Basic compilation test
        assert_eq!(2 + 2, 4);
    }
}
