//! # Printer Event Handler
//!
//! A cross-platform printer status monitoring library for Windows and Linux systems.
//! This library provides functionality to query printer status, monitor printer events, and
//! track printer state changes using platform-specific backends:
//! - **Windows**: WMI (Windows Management Instrumentation) with **complete Win32_Printer support**
//!   - PrinterStatus (current, values 1-7): Other, Unknown, Idle, Printing, Warmup, StoppedPrinting, Offline
//!   - PrinterState (.NET PrintQueueStatus flags): Processing (16384), Printing (1024), Offline (128), etc.
//!   - All 12 DetectedErrorState values (0-11): NoError, NoPaper, Jammed, ServiceRequested, etc.
//! - **Linux**: CUPS (Common Unix Printing System) with basic status detection
//!
//! ## .NET PrintQueueStatus Flags
//!
//! The PrinterState property uses .NET PrintQueueStatus bitwise flags. These flags can be combined,
//! and our implementation uses priority-based parsing to select the most significant state.
//!
//! | Name | Value | Description |
//! |------|-------|-------------|
//! | None | 0 | Status is not specified |
//! | Paused | 1 | The print queue is paused |
//! | Error | 2 | The printer cannot print due to an error condition |
//! | PendingDeletion | 4 | The print queue is deleting a print job |
//! | PaperJam | 8 | The paper in the printer is jammed |
//! | PaperOut | 16 | The printer does not have, or is out of, the type of paper needed for the current print job |
//! | ManualFeed | 32 | The printer is waiting for a user to place print media in the manual feed bin |
//! | PaperProblem | 64 | The paper in the printer is causing an unspecified error condition |
//! | Offline | 128 | The printer is offline |
//! | IOActive | 256 | The printer is exchanging data with the print server |
//! | Busy | 512 | The printer is busy |
//! | Printing | 1024 | The device is printing |
//! | OutputBinFull | 2048 | The printer's output bin is full |
//! | NotAvailable | 4096 | Status information is unavailable |
//! | Waiting | 8192 | The printer is waiting for a print job |
//! | Processing | 16384 | The device is doing some kind of work, which need not be printing if the device also sets Printing |
//! | Initializing | 32768 | The device is in the process of initialization |
//! | WarmingUp | 65536 | The device is warming up |
//! | TonerLow | 131072 | The device is low on toner |
//! | NoToner | 262144 | The device has no toner |
//! | PagePunt | 524288 | The device cannot print the current page |
//! | UserInterventionRequired | 1048576 | The device needs user intervention |
//! | OutOfMemory | 2097152 | The device is out of memory |
//! | DoorOpen | 4194304 | A door on the device is open |
//! | ServerUnknown | 8388608 | Status is unknown |
//! | PowerSave | 16777216 | The device is in power save mode |
//!
//! Reference: [Microsoft .NET PrintQueueStatus](https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus)
//!
//! ## DetectedErrorState Values
//!
//! The DetectedErrorState property indicates specific error conditions on the printer.
//!
//! | Name | Value | Description |
//! |------|-------|-------------|
//! | Unknown | 0 | The error state is unknown |
//! | Other | 1 | An error other than those listed below |
//! | No Error | 2 | The printer is functioning normally |
//! | Low Paper | 3 | The printer is low on paper |
//! | No Paper | 4 | The printer is out of paper |
//! | Low Toner | 5 | The printer is low on toner or ink |
//! | No Toner | 6 | The printer is out of toner or ink |
//! | Door Open | 7 | A door or cover on the printer is open |
//! | Jammed | 8 | Paper is jammed in the printer |
//! | Offline | 9 | The printer is offline |
//! | Service Requested | 10 | The printer requires service or maintenance |
//! | Output Bin Full | 11 | The printer's output bin is full |
//!
//! Reference: [Microsoft Win32_Printer DetectedErrorState](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printer)
//!
//! ## Features
//!
//! - **Comprehensive Windows support** - Full Win32_Printer coverage per Microsoft documentation
//! - **Cross-platform support** (Windows and Linux)
//! - **Real-time monitoring** - Query all printers on the system
//! - **Status change detection** - Monitor specific printers for status changes
//! - **Async/await support** with Tokio
//! - **Detailed status information** - PrinterStatus (current) + PrinterState (.NET flags) and 11 error states
//! - **Platform-specific backends** with unified API
//!
//! ## Cargo features
//!
//! - `rt-multi-thread` *(default)* - enables tokio's multi-threaded runtime. The
//!   library itself only requires tokio's `rt` feature for `tokio::spawn`. The
//!   multi-threaded runtime is on by default so that `#[tokio::main]` works out
//!   of the box. Consumers who prefer the `current_thread` runtime (smaller
//!   binary, embedded use) can opt out with:
//!
//!   ```toml
//!   printer_event_handler = { version = "1.5", default-features = false }
//!   ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use printer_event_handler::{PrinterMonitor, PrinterError, MonitorableProperty};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), PrinterError> {
//!     let monitor = PrinterMonitor::new().await?;
//!     
//!     // List all printers with complete WMI information
//!     let printers = monitor.list_printers().await?;
//!     for printer in printers {
//!         println!("Printer: {}", printer.name());
//!         println!("Status: {}", printer.status_description());
//!         println!("Offline: {}", printer.is_offline());
//!         
//!         // Access raw WMI data
//!         if let Some(code) = printer.printer_status_code() {
//!             println!("  PrinterStatus: {} ({})", code,
//!                 printer.printer_status_description().unwrap_or("Unknown"));
//!         }
//!         
//!         if let Some(status) = printer.wmi_status() {
//!             println!("  WMI Status: {}", status);
//!         }
//!     }
//!     
//!     // Monitor specific property changes with type safety
//!     monitor.monitor_property("HP LaserJet", MonitorableProperty::IsOffline, 60000, None, |change| {
//!         println!("Offline status changed: {}", change.description());
//!     }).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod error;
pub(crate) mod logging;
pub mod monitor;
pub mod printer;

pub use error::PrinterError;
pub use monitor::{MonitorBuilder, MonitorableProperty, PrinterMonitor};
pub use printer::{
    ErrorState, Job, JobStatus, Printer, PrinterChanges, PrinterState, PrinterStatus,
    PropertyChange,
};

// Re-export CancellationToken for convenient access
pub use tokio_util::sync::CancellationToken;

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
