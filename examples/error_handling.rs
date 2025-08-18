//! Error Handling Example
//!
//! This example demonstrates proper error handling patterns with the printer
//! event handler library, including recovery strategies and graceful degradation.
//!
//! Run with: cargo run --example error_handling

use printer_event_handler::{PrinterError, PrinterMonitor};
use std::time::Duration;

#[tokio::main]
async fn main() {
    env_logger::init();

    println!("Printer Event Handler - Error Handling Example");
    println!("===================================================\n");

    // Example 1: Basic error handling with match
    println!("Example 1: Basic Error Handling");
    match initialize_monitor().await {
        Ok(monitor) => {
            println!("   Monitor initialized successfully");
            
            // Try to get printers with error handling
            match get_printers_safely(&monitor).await {
                Ok(count) => println!("   Found {} printers", count),
                Err(e) => println!("   Failed to get printers: {}", e),
            }
        }
        Err(e) => {
            println!("   Failed to initialize monitor: {}", e);
            println!("   This might happen on unsupported platforms");
        }
    }

    println!();

    // Example 2: Error handling with retry logic
    println!("Example 2: Retry Logic with Exponential Backoff");
    match retry_operation().await {
        Ok(_) => println!("   Operation succeeded after retry"),
        Err(e) => println!("   Operation failed after all retries: {}", e),
    }

    println!();

    // Example 3: Graceful degradation
    println!("Example 3: Graceful Degradation");
    graceful_printer_analysis().await;

    println!();

    // Example 4: Specific error type handling
    println!("Example 4: Specific Error Type Handling");
    handle_specific_errors().await;

    println!();

    // Example 5: WMI-specific error handling
    println!("Example 5: WMI-Specific Error Handling");
    handle_wmi_errors().await;
}

/// Initialize monitor with proper error handling
async fn initialize_monitor() -> Result<PrinterMonitor, PrinterError> {
    PrinterMonitor::new().await
}

/// Safely get printers with comprehensive error handling
async fn get_printers_safely(monitor: &PrinterMonitor) -> Result<usize, PrinterError> {
    let printers = monitor.list_printers().await?;
    
    // Validate printer data
    for printer in &printers {
        if printer.name().is_empty() {
            return Err(PrinterError::Other(
                "Found printer with empty name".to_string()
            ));
        }
    }
    
    Ok(printers.len())
}

/// Demonstrate retry logic with exponential backoff
async fn retry_operation() -> Result<(), PrinterError> {
    const MAX_RETRIES: u32 = 3;
    let mut retry_count = 0;
    
    loop {
        match attempt_operation().await {
            Ok(result) => {
                println!("   Operation result: {}", result);
                return Ok(());
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= MAX_RETRIES {
                    return Err(e);
                }
                
                let delay = Duration::from_millis(100 * 2_u64.pow(retry_count));
                println!("   Attempt {} failed: {}. Retrying in {:?}...", 
                    retry_count, e, delay);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Simulate an operation that might fail
async fn attempt_operation() -> Result<String, PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers().await?;
    
    if printers.is_empty() {
        return Err(PrinterError::Other("No printers available".to_string()));
    }
    
    Ok(format!("Found {} printers", printers.len()))
}

/// Demonstrate graceful degradation when some operations fail
async fn graceful_printer_analysis() {
    match PrinterMonitor::new().await {
        Ok(monitor) => {
            println!("   Monitor initialized");
            
            // Try to get detailed printer information, fall back to basic info
            match monitor.list_printers().await {
                Ok(printers) => {
                    println!("   Analyzing {} printers...", printers.len());
                    
                    for printer in printers {
                        println!("      Printer: {}", printer.name());
                        
                        // Basic information (always available)
                        println!("         Status: {}", printer.status_description());
                        
                        // Try to get detailed WMI information (might fail)
                        if let Some(code) = printer.printer_status_code() {
                            if let Some(desc) = printer.printer_status_description() {
                                println!("         PrinterStatus: {} ({})", code, desc);
                            } else {
                                println!("         PrinterStatus: {} (description unavailable)", code);
                            }
                        } else {
                            println!("         PrinterStatus: unavailable");
                        }
                        
                        // Fallback to basic status if detailed info fails
                        match printer.wmi_status() {
                            Some(status) => println!("         WMI Status: {}", status),
                            None => println!("         WMI Status: not available, using basic status"),
                        }
                    }
                }
                Err(e) => {
                    println!("   Could not get printers: {}", e);
                    println!("   Continuing with basic system information...");
                }
            }
        }
        Err(e) => {
            println!("   Monitor initialization failed: {}", e);
            println!("   Possible solutions:");
            println!("      - Check if WMI service is running (Windows)");
            println!("      - Verify CUPS is installed (Linux)");
            println!("      - Run with administrator privileges");
        }
    }
}

/// Handle specific error types with appropriate responses
async fn handle_specific_errors() {
    match PrinterMonitor::new().await {
        Ok(monitor) => {
            // Try to find a non-existent printer
            match monitor.find_printer("NonExistentPrinter123").await {
                Ok(Some(printer)) => {
                    println!("   Unexpectedly found printer: {}", printer.name());
                }
                Ok(None) => {
                    println!("   Correctly handled non-existent printer (returned None)");
                }
                Err(e) => {
                    println!("   Error searching for printer: {}", e);
                    
                    // Handle different error types
                    match e {
                        PrinterError::PlatformNotSupported => {
                            println!("      This platform is not supported");
                        }
                        PrinterError::WmiError(_) => {
                            println!("      WMI access issue - try running as administrator");
                        }
                        PrinterError::CupsError(_) => {
                            println!("      CUPS issue - check if CUPS is running");
                        }
                        PrinterError::PrinterNotFound(name) => {
                            println!("      Printer '{}' not found in system", name);
                        }
                        PrinterError::IoError(io_err) => {
                            println!("      I/O error occurred: {}", io_err);
                        }
                        PrinterError::Other(msg) => {
                            println!("      General error: {}", msg);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("   Could not initialize monitor: {}", e);
        }
    }
}

/// Demonstrate WMI-specific error handling (Windows only)
async fn handle_wmi_errors() {
    #[cfg(windows)]
    {
        println!("   Running Windows-specific WMI error handling...");
        
        match PrinterMonitor::new().await {
            Ok(monitor) => {
                match monitor.list_printers().await {
                    Ok(printers) => {
                        println!("   WMI access successful, found {} printers", printers.len());
                        
                        // Demonstrate accessing WMI properties with error handling
                        for printer in printers.iter().take(1) { // Just check first printer
                            println!("      Checking WMI properties for: {}", printer.name());
                            
                            // Check each WMI property with graceful handling
                            match printer.printer_status_code() {
                                Some(code) => println!("         PrinterStatus: {}", code),
                                None => println!("         PrinterStatus: unavailable"),
                            }
                            
                            match printer.extended_printer_status_code() {
                                Some(code) => println!("         ExtendedPrinterStatus: {}", code),
                                None => println!("         ExtendedPrinterStatus: unavailable"),
                            }
                            
                            match printer.wmi_status() {
                                Some(status) => println!("         WMI Status: \"{}\"", status),
                                None => println!("         WMI Status: unavailable"),
                            }
                        }
                    }
                    Err(PrinterError::WmiError(wmi_err)) => {
                        println!("   WMI Error: {:?}", wmi_err);
                        println!("   Possible solutions:");
                        println!("      - Run as administrator");
                        println!("      - Check if WMI service is running");
                        println!("      - Verify Windows Management Instrumentation service");
                    }
                    Err(e) => {
                        println!("   Other error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("   Failed to initialize WMI monitor: {}", e);
            }
        }
    }
    
    #[cfg(not(windows))]
    {
        println!("   WMI error handling is Windows-specific");
        println!("   On Linux, similar patterns apply to CUPS errors");
    }
}