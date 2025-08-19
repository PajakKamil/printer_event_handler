use crate::backend::{PrinterBackend, create_backend};
use crate::{Printer, Result, PrinterChanges};
use log::{error, info, warn};
use std::collections::HashMap;
use tokio::time::{Duration, sleep};

/// Enum representing all available printer properties that can be monitored.
///
/// This enum provides type-safe access to all printer properties that can be
/// monitored for changes, replacing string-based property names with a
/// strongly-typed interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorableProperty {
    /// Printer name changes
    Name,
    /// PrinterStatus enum changes (recommended current status)
    Status,
    /// PrinterState enum changes (legacy Windows state)
    State,
    /// ErrorState enum changes
    ErrorState,
    /// Online/offline status changes
    IsOffline,
    /// Default printer designation changes
    IsDefault,
    /// Raw PrinterStatus code changes (1-7)
    PrinterStatusCode,
    /// Raw PrinterState code changes (.NET flags)
    PrinterStateCode,
    /// Raw DetectedErrorState code changes (0-11)
    DetectedErrorStateCode,
    /// Raw ExtendedDetectedErrorState code changes
    ExtendedDetectedErrorStateCode,
    /// Raw ExtendedPrinterStatus code changes
    ExtendedPrinterStatusCode,
    /// WMI Status property changes ("OK", "Error", etc.)
    WmiStatus,
}

impl MonitorableProperty {
    /// Returns the string representation of the property name.
    ///
    /// This matches the property names used in the PropertyChange enum.
    pub fn as_str(&self) -> &'static str {
        match self {
            MonitorableProperty::Name => "Name",
            MonitorableProperty::Status => "Status",
            MonitorableProperty::State => "State",
            MonitorableProperty::ErrorState => "ErrorState",
            MonitorableProperty::IsOffline => "IsOffline",
            MonitorableProperty::IsDefault => "IsDefault",
            MonitorableProperty::PrinterStatusCode => "PrinterStatusCode",
            MonitorableProperty::PrinterStateCode => "PrinterStateCode",
            MonitorableProperty::DetectedErrorStateCode => "DetectedErrorStateCode",
            MonitorableProperty::ExtendedDetectedErrorStateCode => "ExtendedDetectedErrorStateCode",
            MonitorableProperty::ExtendedPrinterStatusCode => "ExtendedPrinterStatusCode",
            MonitorableProperty::WmiStatus => "WmiStatus",
        }
    }

    /// Returns a human-readable description of what this property represents.
    pub fn description(&self) -> &'static str {
        match self {
            MonitorableProperty::Name => "Printer name",
            MonitorableProperty::Status => "Current printer status (recommended)",
            MonitorableProperty::State => "Printer state (legacy Windows property)",
            MonitorableProperty::ErrorState => "Current error condition",
            MonitorableProperty::IsOffline => "Online/offline status",
            MonitorableProperty::IsDefault => "Default printer designation",
            MonitorableProperty::PrinterStatusCode => "Raw printer status code (1-7)",
            MonitorableProperty::PrinterStateCode => "Raw printer state code (.NET flags)",
            MonitorableProperty::DetectedErrorStateCode => "Raw detected error state code (0-11)",
            MonitorableProperty::ExtendedDetectedErrorStateCode => "Extended error state code",
            MonitorableProperty::ExtendedPrinterStatusCode => "Extended printer status code",
            MonitorableProperty::WmiStatus => "WMI status property",
        }
    }

    /// Returns all available properties that can be monitored.
    pub fn all() -> Vec<MonitorableProperty> {
        vec![
            MonitorableProperty::Name,
            MonitorableProperty::Status,
            MonitorableProperty::State,
            MonitorableProperty::ErrorState,
            MonitorableProperty::IsOffline,
            MonitorableProperty::IsDefault,
            MonitorableProperty::PrinterStatusCode,
            MonitorableProperty::PrinterStateCode,
            MonitorableProperty::DetectedErrorStateCode,
            MonitorableProperty::ExtendedDetectedErrorStateCode,
            MonitorableProperty::ExtendedPrinterStatusCode,
            MonitorableProperty::WmiStatus,
        ]
    }
}

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
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     
    ///     monitor.monitor_printer("HP LaserJet", 30000, |current, previous| {
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
                    println!("[{}] Checking printer: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"), current_printer.name());
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

            sleep(Duration::from_millis(interval_ms)).await;
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

    /// Monitors a printer with detailed property change detection.
    ///
    /// This enhanced monitoring method provides detailed information about exactly which
    /// properties changed between checks, enabling fine-grained monitoring and alerting.
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `callback` - Function called when properties change, receives PrinterChanges
    ///
    /// # Returns
    /// * `Result<()>` - Never returns Ok normally (runs indefinitely), only Err on failure
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     
    ///     monitor.monitor_printer_changes("HP LaserJet", 30000, |changes| {
    ///         if changes.has_changes() {
    ///             println!("Detected {} changes:", changes.change_count());
    ///             for change in &changes.changes {
    ///                 println!("  - {}", change.description());
    ///             }
    ///         }
    ///     }).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_printer_changes<F>(
        &self,
        printer_name: &str,
        interval_ms: u64,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&PrinterChanges) + Send,
    {
        info!("Starting detailed printer change monitoring for: {}", printer_name);

        let mut previous_printer: Option<Printer> = None;

        loop {
            match self.find_printer(printer_name).await {
                Ok(Some(current_printer)) => {
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
                        // Initial state - report as "initial" (no previous state)
                        let changes = PrinterChanges::new(current_printer.name().to_string());
                        callback(&changes);
                        info!("Printer '{}' - Initial state captured", printer_name);
                    }
                    previous_printer = Some(current_printer);
                }
                Ok(None) => {
                    warn!("Printer '{}' not found", printer_name);
                    if let Some(prev) = previous_printer.take() {
                        // Printer disappeared - create a change showing it went offline
                        let mut changes = PrinterChanges::new(printer_name.to_string());
                        changes.changes.push(crate::PropertyChange::IsOffline {
                            old: prev.is_offline(),
                            new: true,
                        });
                        callback(&changes);
                    }
                }
                Err(e) => {
                    error!("Failed to check printer status: {}", e);
                    return Err(e);
                }
            }

            sleep(Duration::from_millis(interval_ms)).await;
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
    /// * `callback` - Function called when the property changes
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::{PrinterMonitor, MonitorableProperty};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     
    ///     monitor.monitor_property("HP LaserJet", MonitorableProperty::IsOffline, 60000, |change| {
    ///         println!("Offline status changed: {}", change.description());
    ///     }).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_property<F>(
        &self,
        printer_name: &str,
        property: MonitorableProperty,
        interval_ms: u64,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&crate::PropertyChange) + Send,
    {
        let property_name = property.as_str();
        info!("Starting property '{}' monitoring for printer: {}", property_name, printer_name);

        self.monitor_printer_changes(printer_name, interval_ms, move |changes| {
            for change in &changes.changes {
                if change.property_name() == property_name {
                    callback(change);
                }
            }
        }).await
    }

    /// Monitors multiple printers concurrently and reports changes for any of them.
    ///
    /// This method allows monitoring several printers simultaneously, with a single
    /// callback that receives changes from any of the monitored printers.
    ///
    /// # Arguments
    /// * `printer_names` - List of printer names to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `callback` - Function called when any printer changes
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let printers = vec!["HP LaserJet".to_string(), "Canon Printer".to_string()];
    ///     
    ///     monitor.monitor_multiple_printers(printers, 30000, |changes| {
    ///         println!("Printer '{}' changed: {}", changes.printer_name, changes.summary());
    ///     }).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_multiple_printers<F>(
        &self,
        printer_names: Vec<String>,
        interval_ms: u64,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(&PrinterChanges) + Send + Sync + 'static,
    {
        use std::sync::Arc;
        use tokio::task::JoinHandle;

        info!("Starting concurrent monitoring of {} printers", printer_names.len());
        
        let callback = Arc::new(callback);
        let mut tasks: Vec<JoinHandle<Result<()>>> = Vec::new();

        for printer_name in printer_names {
            let callback_clone = callback.clone();
            let printer_name_clone = printer_name.clone();

            let task = tokio::spawn(async move {
                // This is a bit tricky - we can't easily clone self, so we need to create a new monitor
                // In practice, you'd want to refactor this to share the backend more efficiently
                let new_monitor = PrinterMonitor::new().await?;
                new_monitor.monitor_printer_changes(&printer_name_clone, interval_ms, move |changes| {
                    callback_clone(changes);
                }).await
            });

            tasks.push(task);
        }

        // Wait for all monitoring tasks (this will run indefinitely unless one fails)
        for task in tasks {
            match task.await {
                Ok(Ok(())) => {
                    info!("Monitoring task completed successfully");
                }
                Ok(Err(e)) => {
                    error!("Monitoring task failed: {}", e);
                    return Err(e);
                }
                Err(e) => {
                    error!("Monitoring task panicked: {}", e);
                    return Err(crate::PrinterError::Other(format!("Task panicked: {}", e)));
                }
            }
        }

        Ok(())
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
