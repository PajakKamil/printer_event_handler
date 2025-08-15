use log::error;
use printer_event_handler::{PrinterError, PrinterMonitor};
use std::env;

/// Monitors a specific printer and displays status changes in the CLI.
///
/// This function implements the monitoring mode of the CLI application,
/// continuously checking the specified printer every 60 seconds and
/// displaying any status changes with timestamps.
///
/// # Arguments
/// * `printer_name` - The name of the printer to monitor
///
/// # Returns
/// * `Result<(), PrinterError>` - Ok if monitoring completes successfully, Err on failure
///
/// # Errors
/// * `PrinterError::PrinterNotFound` - If the specified printer doesn't exist
/// * `PrinterError::WmiError` - If WMI queries fail on Windows
/// * `PrinterError::CupsError` - If CUPS queries fail on Linux
/// * `PrinterError::PlatformNotSupported` - If running on an unsupported platform
async fn monitor_printer_cli(printer_name: &str) -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;

    println!("Printer Status Monitor Service");
    println!("==============================");
    println!("Monitoring printer '{}' every 60 seconds...", printer_name);
    println!("Press Ctrl+C to stop\n");

    monitor
        .monitor_printer(printer_name, 60, |current, previous| {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

            if let Some(prev) = previous {
                if prev != current {
                    println!(
                        "[{}] Printer '{}' Status Changed:",
                        timestamp,
                        current.name()
                    );
                    println!(
                        "  Status: {} -> {}",
                        prev.status_description(),
                        current.status_description()
                    );
                    println!(
                        "  Error State: {} -> {}",
                        prev.error_description(),
                        current.error_description()
                    );
                } else {
                    println!("[{}] Checking printer '{}'", timestamp, current.name());
                }
            } else {
                println!(
                    "[{}] Printer '{}' Initial Status:",
                    timestamp,
                    current.name()
                );
                println!("  Status: {}", current.status_description());
                println!("  Error State: {}", current.error_description());
            }

            println!(
                "  Offline: {}",
                if current.is_offline() { "Yes" } else { "No" }
            );
            println!();
        })
        .await?;

    Ok(())
}

/// Lists all printers on the system in a formatted CLI display.
///
/// This function implements the list mode of the CLI application,
/// querying all available printers and displaying their current
/// status information in a user-friendly format.
///
/// # Returns
/// * `Result<(), PrinterError>` - Ok if listing completes successfully, Err on failure
///
/// # Errors
/// * `PrinterError::WmiError` - If WMI queries fail on Windows
/// * `PrinterError::CupsError` - If CUPS queries fail on Linux
/// * `PrinterError::PlatformNotSupported` - If running on an unsupported platform
/// * `PrinterError::IoError` - If there are system I/O issues
async fn list_printers_cli() -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers().await?;

    if printers.is_empty() {
        println!("No printers found on this system.");
    } else {
        println!("Found {} printer(s):\n", printers.len());

        for (i, printer) in printers.iter().enumerate() {
            println!("Printer #{}: {}", i + 1, printer.name());
            println!("  Status: {}", printer.status_description());
            println!("  Error State: {}", printer.error_description());
            println!(
                "  Offline: {}",
                if printer.is_offline() { "Yes" } else { "No" }
            );

            if printer.is_default() {
                println!("  Default Printer: Yes");
            }

            println!();
        }
    }

    Ok(())
}

/// Main entry point for the printer monitoring CLI application.
///
/// This function handles command-line argument parsing and dispatches to
/// either list mode (no arguments) or monitor mode (printer name provided).
/// It also sets up logging and handles platform-specific error reporting.
///
/// # Command Line Usage
/// * No arguments: Lists all printers once and exits
/// * One argument: Monitors the named printer continuously
///
/// # Returns
/// * `Result<(), Box<dyn std::error::Error>>` - Ok on successful completion, Err on failure
///
/// # Examples
/// ```bash
/// # List all printers
/// cargo run
///
/// # Monitor a specific printer
/// cargo run -- "HP LaserJet Pro"
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let printer_name = &args[1];

        match monitor_printer_cli(printer_name).await {
            Ok(()) => {}
            Err(PrinterError::PlatformNotSupported) => {
                println!("This application only supports Windows systems.");
                println!("Printer monitoring requires Windows Management Instrumentation (WMI).");
            }
            Err(e) => {
                error!("Failed to monitor printer: {}", e);
                println!("Error: {}", e);
                return Err(e.into());
            }
        }
    } else {
        println!("Printer Status Checker");
        println!("======================");
        println!("Usage:");
        println!(
            "  {} <printer_name>    Monitor specific printer every 60 seconds",
            args[0]
        );
        println!("  {}                   List all printers once\n", args[0]);

        match list_printers_cli().await {
            Ok(()) => {}
            Err(PrinterError::PlatformNotSupported) => {
                println!("This application only supports Windows systems.");
                println!(
                    "Printer status checking requires Windows Management Instrumentation (WMI)."
                );
            }
            Err(e) => {
                error!("Failed to list printers: {}", e);
                println!("Error: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}
