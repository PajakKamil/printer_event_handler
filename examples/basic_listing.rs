//! Basic Printer Listing Example
//!
//! This example demonstrates how to list all printers on the system with
//! complete WMI information access including status codes and descriptions.
//!
//! Run with: cargo run --example basic_listing

use printer_event_handler::{PrinterError, PrinterMonitor};

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    // Initialize logging
    env_logger::init();

    println!("Printer Event Handler - Basic Listing Example");
    println!("==================================================\n");

    // Create printer monitor
    let monitor = PrinterMonitor::new().await?;

    // Get all printers
    let printers = monitor.list_printers().await?;

    if printers.is_empty() {
        println!("No printers found on this system.");
        return Ok(());
    }

    println!("Found {} printer(s):\n", printers.len());

    // Display each printer with complete information
    for (i, printer) in printers.iter().enumerate() {
        println!("Printer #{}: {}", i + 1, printer.name());

        // High-level status information
        println!("   Status: {}", printer.status_description());
        println!("   Error State: {}", printer.error_description());
        println!(
            "   Offline: {}",
            if printer.is_offline() { "Yes" } else { "No" }
        );

        if printer.is_default() {
            println!("   Default Printer: Yes");
        }

        // Detailed WMI information
        println!("\n   Complete WMI Status Information:");

        // PrinterStatus (current/recommended property)
        if let Some(code) = printer.printer_status_code() {
            println!(
                "   └── PrinterStatus: {} ({})",
                code,
                printer.printer_status_description().unwrap_or("Unknown")
            );
        }

        // PrinterState (obsolete but detailed property)
        if let Some(code) = printer.printer_state_code() {
            println!(
                "   └── PrinterState: {} ({})",
                code,
                printer.printer_state_description().unwrap_or("Unknown")
            );
        }

        // DetectedErrorState
        if let Some(code) = printer.detected_error_state_code() {
            println!(
                "   └── DetectedErrorState: {} ({})",
                code,
                printer
                    .detected_error_state_description()
                    .unwrap_or("Unknown")
            );
        }

        // ExtendedPrinterStatus
        if let Some(code) = printer.extended_printer_status_code() {
            println!(
                "   └── ExtendedPrinterStatus: {} ({})",
                code,
                printer
                    .extended_printer_status_description()
                    .unwrap_or("Unknown")
            );
        }

        // ExtendedDetectedErrorState
        if let Some(code) = printer.extended_detected_error_state_code() {
            println!("   └── ExtendedDetectedErrorState: {}", code);
        }

        // WMI Status property
        if let Some(status) = printer.wmi_status() {
            println!("   └── WMI Status: \"{}\"", status);
        }

        // Optional: Access individual state enums
        if let Some(state) = printer.state() {
            println!("   └── PrinterState Enum: {}", state.description());
        }

        println!(); // Empty line between printers
    }

    // Summary information
    let online_count = printers.iter().filter(|p| !p.is_offline()).count();
    let offline_count = printers.len() - online_count;
    let error_count = printers.iter().filter(|p| p.has_error()).count();

    println!("Summary:");
    println!("   Total printers: {}", printers.len());
    println!("   Online: {}", online_count);
    println!("   Offline: {}", offline_count);
    println!("   With errors: {}", error_count);

    Ok(())
}
