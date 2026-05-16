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

/// Runs the Win32_Printer query reusing a thread-local `WMIConnection`.
///
/// `wmi::WMIConnection` is `!Send + !Sync` because it holds COM state tied to
/// the thread that created it, so the previous code paid the COM init cost on
/// every `list_printers` call. Storing the connection in a `thread_local!` lets
/// each tokio blocking-pool thread initialise it once and reuse it for the
/// lifetime of the thread. On query failure the cache is cleared so the next
/// call rebuilds the connection - covers WMI service restarts and stale
/// handles.
#[cfg(windows)]
fn query_win32_printers_cached() -> Result<Vec<crate::printer::Win32Printer>> {
    use crate::printer::Win32Printer;
    use std::cell::RefCell;

    thread_local! {
        static WMI_CONNECTION: RefCell<Option<wmi::WMIConnection>> = const { RefCell::new(None) };
    }

    WMI_CONNECTION.with(|cell| -> Result<Vec<Win32Printer>> {
        // Lazy init: scope the mutable borrow so it's released before the query.
        {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(wmi::WMIConnection::new().map_err(PrinterError::from)?);
            }
        }

        // SELECT * because WQL treats "Default" as a reserved keyword.
        let query_result = {
            let slot = cell.borrow();
            let conn = slot.as_ref().expect("connection initialised above");
            conn.raw_query::<Win32Printer>("SELECT * FROM Win32_Printer")
        };

        match query_result {
            Ok(printers) => Ok(printers),
            Err(e) => {
                // Drop the (possibly stale) cached connection so the next call
                // can recover from WMI service restarts or expired sessions.
                *cell.borrow_mut() = None;
                Err(PrinterError::from(e))
            }
        }
    })
}

#[cfg(windows)]
#[async_trait]
impl PrinterBackend for WindowsBackend {
    async fn new() -> Result<Self> {
        use log::info;

        info!("Initializing Windows WMI backend...");
        Ok(Self)
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        use log::info;

        info!("Querying printer information via WMI...");

        let wmi_printers = tokio::task::spawn_blocking(query_win32_printers_cached)
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
                if let Some(name) = line.strip_prefix("system default destination: ") {
                    return Some(name.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_backend() {
        let result = create_backend().await;

        #[cfg(any(windows, unix))]
        {
            // On supported platforms, backend creation should succeed
            assert!(result.is_ok());
        }

        #[cfg(not(any(windows, unix)))]
        {
            // On unsupported platforms, should return error
            assert!(matches!(result, Err(PrinterError::PlatformNotSupported)));
        }
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn test_windows_backend_creation() {
        let result = WindowsBackend::new().await;
        // May fail in test environments without WMI access, but should not panic
        match result {
            Ok(_) => println!("Windows backend created successfully"),
            Err(e) => println!("Expected error in test environment: {}", e),
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_linux_backend_creation() {
        let result = LinuxBackend::new().await;
        // Should succeed even without CUPS, as we have fallback detection
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn test_windows_backend_list_printers() {
        let backend_result = WindowsBackend::new().await;
        if let Ok(backend) = backend_result {
            let printers = backend.list_printers().await;
            // Either returns printers or an error, but shouldn't panic
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers via Windows backend", printer_list.len());
                    // Verify each printer has required fields
                    for printer in printer_list {
                        assert!(!printer.name().is_empty());
                    }
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_linux_backend_list_printers() {
        let backend = LinuxBackend::new().await;
        assert!(backend.is_ok());

        if let Ok(backend) = backend {
            let printers = backend.list_printers().await;
            // Should return result, even if empty
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers via Linux backend", printer_list.len());
                    // Verify each printer has required fields
                    for printer in printer_list {
                        assert!(!printer.name().is_empty());
                    }
                }
                Err(e) => {
                    println!("Error listing printers: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_linux_backend_find_printer() {
        let backend = LinuxBackend::new().await;
        assert!(backend.is_ok());

        if let Ok(backend) = backend {
            // Try to find a non-existent printer
            let result = backend.find_printer("NonExistentPrinter_Test_12345").await;
            assert!(result.is_ok());
            // Should return None for non-existent printer
            if let Ok(printer_opt) = result {
                if printer_opt.is_some() {
                    println!("Warning: Found printer with unlikely name");
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_idle() {
        let line = "printer HP_LaserJet is idle.  enabled since Mon 01 Jan 2024 12:00:00 PM UTC";
        let printer = parse_lpstat_line(line);

        assert!(printer.is_some());
        if let Some(p) = printer {
            assert_eq!(p.name(), "HP_LaserJet");
            assert_eq!(p.status(), &crate::PrinterStatus::Idle);
            assert_eq!(p.error_state(), &crate::ErrorState::NoError);
            assert!(!p.is_offline());
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_printing() {
        let line =
            "printer Canon_MX920 is printing.  enabled since Tue 02 Jan 2024 10:00:00 AM UTC";
        let printer = parse_lpstat_line(line);

        assert!(printer.is_some());
        if let Some(p) = printer {
            assert_eq!(p.name(), "Canon_MX920");
            assert_eq!(p.status(), &crate::PrinterStatus::Printing);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_stopped() {
        let line = "printer Epson_XP is stopped.  disabled since Wed 03 Jan 2024 08:00:00 AM UTC";
        let printer = parse_lpstat_line(line);

        assert!(printer.is_some());
        if let Some(p) = printer {
            assert_eq!(p.name(), "Epson_XP");
            assert_eq!(p.status(), &crate::PrinterStatus::Offline);
            assert!(p.is_offline());
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_invalid() {
        let line = "This is not a valid lpstat line";
        let printer = parse_lpstat_line(line);
        assert!(printer.is_none());
    }
}
