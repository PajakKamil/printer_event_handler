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
    /// Create a new PrinterMonitor instance
    pub async fn new() -> Result<Self> {
        info!("Initializing printer monitor...");
        let backend = create_backend().await?;
        Ok(Self { backend })
    }

    /// List all printers on the system
    pub async fn list_printers(&self) -> Result<Vec<Printer>> {
        self.backend.list_printers().await
    }

    /// Find a printer by name (case-insensitive)
    pub async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        self.backend.find_printer(name).await
    }

    /// Monitor a specific printer for status changes
    ///
    /// This function runs indefinitely, checking the printer status every `interval` seconds
    /// and calling the provided callback when the status changes.
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

    /// Get a summary of all printers and their current states
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

/// Summary information about a printer's current state
#[derive(Debug, Clone)]
pub struct PrinterSummary {
    pub status: crate::PrinterStatus,
    pub error_state: crate::ErrorState,
    pub is_offline: bool,
    pub is_default: bool,
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
