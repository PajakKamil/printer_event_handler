//! # Printer Event Handler
//!
//! A cross-platform printer status monitoring library for Windows and Linux systems.
//! This library provides functionality to query printer status, monitor printer events, and
//! track printer state changes using platform-specific backends:
//! - Windows: WMI (Windows Management Instrumentation)  
//! - Linux: CUPS (Common Unix Printing System)
//!
//! ## Features
//!
//! - Cross-platform support (Windows and Linux)
//! - Query all printers on the system
//! - Monitor specific printers for status changes
//! - Async/await support with Tokio
//! - Detailed printer status and error state information
//! - Platform-specific backends with unified API
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
    use super::*;

    #[test]
    fn test_library_compiles() {
        // Basic compilation test
        assert_eq!(2 + 2, 4);
    }
}
