#[cfg(windows)]
use crate::PrinterError;
use crate::{Printer, Result};
use async_trait::async_trait;

/// Trait for platform-specific printer backend implementations
#[async_trait]
pub trait PrinterBackend: Send + Sync {
    /// Initialize the backend
    async fn new() -> Result<Self>
    where
        Self: Sized;

    /// List all printers on the system
    async fn list_printers(&self) -> Result<Vec<Printer>>;

    /// Find a printer by name (case-insensitive)
    async fn find_printer(&self, name: &str) -> Result<Option<Printer>>;
}

/// Windows backend using WMI
#[cfg(windows)]
pub struct WindowsBackend;

#[cfg(windows)]
#[async_trait]
impl PrinterBackend for WindowsBackend {
    async fn new() -> Result<Self> {
        use log::info;

        info!("Initializing Windows WMI backend...");
        Ok(Self)
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        use crate::printer::Win32Printer;
        use log::info;
        use wmi::COMLibrary;

        info!("Querying printer information via WMI...");

        // Run WMI operations in a blocking task to avoid Send/Sync issues
        let wmi_printers = tokio::task::spawn_blocking(|| -> Result<Vec<Win32Printer>> {
            let com_con = COMLibrary::new().map_err(PrinterError::from)?;
            let wmi_connection = wmi::WMIConnection::new(com_con).map_err(PrinterError::from)?;
            let printers: Vec<Win32Printer> = wmi_connection.raw_query("SELECT Name, PrinterStatus, DetectedErrorState, WorkOffline, PrinterState, Default, ExtendedPrinterStatus, ExtendedDetectedErrorState, Status FROM Win32_Printer").map_err(PrinterError::from)?;
            Ok(printers)
        })
        .await
        .map_err(|e| PrinterError::Other(format!("Failed to execute WMI query: {}", e)))??;

        let printers = wmi_printers.into_iter().map(Printer::from).collect();
        Ok(printers)
    }

    async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        let printers = self.list_printers().await?;

        for printer in printers {
            if printer.name().eq_ignore_ascii_case(name) {
                return Ok(Some(printer));
            }
        }

        Ok(None)
    }
}

/// Linux backend using CUPS commands
#[cfg(unix)]
pub struct LinuxBackend;

#[cfg(unix)]
#[async_trait]
impl PrinterBackend for LinuxBackend {
    async fn new() -> Result<Self> {
        use log::info;
        use tokio::process::Command;

        info!("Initializing Linux CUPS backend...");

        // Check if lpstat is available
        let output = Command::new("which").arg("lpstat").output().await;

        match output {
            Ok(result) if result.status.success() => {
                info!("CUPS tools found, backend ready");
                Ok(Self)
            }
            _ => {
                // Check if we can find any printers using /proc or /sys
                info!("CUPS not found, checking for alternative printer detection methods");
                Ok(Self)
            }
        }
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        use log::{info, warn};
        use tokio::process::Command;

        info!("Querying printer information via system commands...");

        let mut printers = Vec::new();

        // Try lpstat first
        if let Ok(output) = Command::new("lpstat").arg("-p").arg("-d").output().await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);

                for line in stdout.lines() {
                    if line.starts_with("printer ") {
                        if let Some(printer_info) = parse_lpstat_line(line) {
                            printers.push(printer_info);
                        }
                    }
                }

                // Get default printer
                let default_printer = get_default_printer().await;

                // Mark default printer
                if let Some(ref default_name) = default_printer {
                    for printer in &mut printers {
                        if printer.name() == default_name {
                            *printer = Printer::new(
                                printer.name().to_string(),
                                printer.status().clone(),
                                printer.error_state().clone(),
                                printer.is_offline(),
                                true, // is_default
                            );
                        }
                    }
                }
            }
        }

        // If no printers found via lpstat, try alternative methods
        if printers.is_empty() {
            warn!("No printers found via lpstat, trying alternative detection methods");
            printers.extend(detect_printers_alternative().await?);
        }

        Ok(printers)
    }

    async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        let printers = self.list_printers().await?;

        for printer in printers {
            if printer.name().eq_ignore_ascii_case(name) {
                return Ok(Some(printer));
            }
        }

        Ok(None)
    }
}

#[cfg(unix)]
fn parse_lpstat_line(line: &str) -> Option<Printer> {
    use crate::{ErrorState, PrinterStatus};

    // Example line: "printer HP_LaserJet_1020 is idle.  enabled since Mon 01 Jan 2024 12:00:00 PM UTC"
    if let Some(rest) = line.strip_prefix("printer ") {
        if let Some(space_pos) = rest.find(' ') {
            let name = &rest[..space_pos];
            let status_part = &rest[space_pos + 1..];

            let (status, error_state, is_offline) = if status_part.contains("idle") {
                (PrinterStatus::Idle, ErrorState::NoError, false)
            } else if status_part.contains("printing") {
                (PrinterStatus::Printing, ErrorState::NoError, false)
            } else if status_part.contains("stopped") || status_part.contains("disabled") {
                (PrinterStatus::Offline, ErrorState::Other, true)
            } else {
                (
                    PrinterStatus::StatusUnknown,
                    ErrorState::UnknownError,
                    false,
                )
            };

            return Some(Printer::new(
                name.to_string(),
                status,
                error_state,
                is_offline,
                false, // is_default - will be set later
            ));
        }
    }

    None
}

#[cfg(unix)]
async fn get_default_printer() -> Option<String> {
    use tokio::process::Command;

    if let Ok(output) = Command::new("lpstat").arg("-d").output().await {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with("system default destination: ") {
                    return Some(line.replace("system default destination: ", ""));
                }
                if line.starts_with("no system default destination") {
                    return None;
                }
            }
        }
    }

    None
}

#[cfg(unix)]
async fn detect_printers_alternative() -> Result<Vec<Printer>> {
    use crate::{ErrorState, PrinterStatus};
    use log::info;
    use tokio::fs;

    let mut printers = Vec::new();

    // Check for USB printers in /sys/class/usb
    info!("Checking for USB printers in /sys/class/usb...");
    if let Ok(_entries) = fs::read_dir("/sys/class/usb").await {
        // This is a basic implementation - in practice you'd need to parse USB device info
        // to identify printers by their device class
        info!("Found USB entries, but printer detection requires more complex parsing");
    }

    // Check for parallel port printers
    if let Ok(_) = fs::metadata("/dev/lp0").await {
        info!("Found parallel port printer device");
        printers.push(Printer::new(
            "Parallel Port Printer".to_string(),
            PrinterStatus::StatusUnknown,
            ErrorState::UnknownError,
            false,
            false,
        ));
    }

    // For WSL or systems without direct hardware access, we might not find any printers
    if printers.is_empty() {
        info!("No printers detected via alternative methods");
    }

    Ok(printers)
}

/// Create the appropriate backend for the current platform
pub async fn create_backend() -> Result<Box<dyn PrinterBackend>> {
    #[cfg(windows)]
    {
        let backend = WindowsBackend::new().await?;
        Ok(Box::new(backend))
    }

    #[cfg(unix)]
    {
        let backend = LinuxBackend::new().await?;
        Ok(Box::new(backend))
    }

    #[cfg(not(any(windows, unix)))]
    {
        Err(PrinterError::PlatformNotSupported)
    }
}
