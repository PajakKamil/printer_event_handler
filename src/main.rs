
use printer_event_handler::{PrinterMonitor, PrinterError};
use log::error;
use std::env;

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
                    println!("[{}] Printer '{}' Status Changed:", timestamp, current.name());
                    println!("  Status: {} -> {}", prev.status_description(), current.status_description());
                    println!("  Error State: {} -> {}", prev.error_description(), current.error_description());
                } else {
                    println!("[{}] Checking printer '{}'", timestamp, current.name());
                }
            } else {
                println!("[{}] Printer '{}' Initial Status:", timestamp, current.name());
                println!("  Status: {}", current.status_description());
                println!("  Error State: {}", current.error_description());
            }
            
            println!("  Offline: {}", if current.is_offline() { "Yes" } else { "No" });
            println!();
        })
        .await?;
    
    Ok(())
}

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
            println!("  Offline: {}", if printer.is_offline() { "Yes" } else { "No" });
            
            if printer.is_default() {
                println!("  Default Printer: Yes");
            }
            
            println!();
        }
    }
    
    Ok(())
}

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
        println!("  {} <printer_name>    Monitor specific printer every 60 seconds", args[0]);
        println!("  {}                   List all printers once\n", args[0]);
        
        match list_printers_cli().await {
            Ok(()) => {}
            Err(PrinterError::PlatformNotSupported) => {
                println!("This application only supports Windows systems.");
                println!("Printer status checking requires Windows Management Instrumentation (WMI).");
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
