use crate::backend::{create_backend, PrinterBackend};
use crate::{Printer, Result};
use log::{error, info, warn};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

/// Printer monitoring and querying functionality
pub struct PrinterMonitor {
    backend: Box<dyn PrinterBackend>,
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
        Ok(Self { backend })
    }

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
    pub async fn list_printers(&self) -> Result<Vec<Printer>> {
        self.backend.list_printers().await
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
    pub async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        self.backend.find_printer(name).await
    }

    /// Continuously monitors a specific printer for status changes.
    ///
    /// This function runs indefinitely, polling the specified printer every `interval_secs`
    /// seconds and calling the provided callback function whenever the printer's status changes.
    /// The callback receives both the current printer state and the previous state (if any).
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `interval_secs` - Polling interval in seconds
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
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     
    ///     monitor.monitor_printer("HP LaserJet", 30, |current, previous| {
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
        interval_secs: u64,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&Printer, Option<&Printer>) + Send,
    {
        info!("Starting printer monitoring service for: {}", printer_name);

        let mut previous_printer: Option<Printer> = None;

        loop {
            match self.find_printer(printer_name).await {
                Ok(Some(current_printer)) => {
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
                    error!("Failed to check printer status: {}", e);
                    return Err(e);
                }
            }

            sleep(Duration::from_secs(interval_secs)).await;
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
        let printers = self.list_printers().await?;
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
}

/// Summary information about a printer's current state.
///
/// This struct provides a snapshot of a printer's essential status information
/// in a convenient format for reporting and monitoring applications.
#[derive(Debug, Clone)]
pub struct PrinterSummary {
    /// Current operational status of the printer
    pub status: crate::PrinterStatus,
    /// Current error state of the printer
    pub error_state: crate::ErrorState,
    /// Whether the printer is currently offline
    pub is_offline: bool,
    /// Whether this is the system's default printer
    pub is_default: bool,
    /// Whether the printer currently has any error conditions
    pub has_error: bool,
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
}
