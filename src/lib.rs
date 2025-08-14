//! # Printer Event Handler
//!
//! A Windows printer status monitoring library using WMI (Windows Management Instrumentation).
//! This library provides functionality to query printer status, monitor printer events, and
//! track printer state changes on Windows systems.
//!
//! ## Features
//!
//! - Query all printers on the system
//! - Monitor specific printers for status changes
//! - Async/await support with Tokio
//! - Detailed printer status and error state information
//! - Windows-only with proper compilation guards
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

pub mod error;
pub mod monitor;
pub mod printer;

pub use error::PrinterError;
pub use monitor::PrinterMonitor;
pub use printer::{Printer, PrinterStatus, ErrorState};

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