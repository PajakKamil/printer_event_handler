//! Printer Status Change Monitoring Example
//!
//! This example demonstrates how to monitor a specific printer for status changes
//! and display detailed WMI information when changes occur.
//!
//! Run with: cargo run --example monitor_changes -- "Printer Name"
//! Or: cargo run --example monitor_changes (for first available printer)

use printer_event_handler::{PrinterError, PrinterMonitor};
use std::env;

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Status Change Monitor");
    println!("==================================================\n");

    let monitor = PrinterMonitor::new().await?;

    // Get printer name from command line or use first available
    let printer_name = env::args().nth(1).unwrap_or_else(|| {
        println!("No printer name provided. Looking for first available printer...");
        String::new()
    });

    let target_printer_name = if printer_name.is_empty() {
        // Find first available printer
        let printers = monitor.list_printers().await?;
        if printers.is_empty() {
            println!("No printers found on this system.");
            return Ok(());
        }
        println!("Using first available printer: {}\n", printers[0].name());
        printers[0].name().to_string()
    } else {
        println!("Monitoring printer: {}\n", printer_name);
        printer_name
    };

    // Verify printer exists
    if monitor.find_printer(&target_printer_name).await?.is_none() {
        println!("Printer '{}' not found!", target_printer_name);
        return Ok(());
    }

    println!("Starting printer monitoring...");
    println!("   Checking every 1 second for changes");
    println!("   Press Ctrl+C to stop\n");

    // Monitor printer with detailed change reporting
    monitor
        .monitor_printer(&target_printer_name, 1000, |current, previous| {
            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
            if let Some(prev) = previous {
                if prev != current {
                    println!("[{}] PRINTER STATUS CHANGED!", timestamp);
                    println!("Printer: {}", current.name());

                    // Compare high-level changes
                    if prev.status() != current.status() {
                        println!(
                            "   Status: {} → {}",
                            prev.status_description(),
                            current.status_description()
                        );
                    }

                    if prev.error_state() != current.error_state() {
                        println!(
                            "   Error State: {} → {}",
                            prev.error_description(),
                            current.error_description()
                        );
                    }

                    if prev.is_offline() != current.is_offline() {
                        println!(
                            "   Offline: {} → {}",
                            if prev.is_offline() { "Yes" } else { "No" },
                            if current.is_offline() { "Yes" } else { "No" }
                        );
                    }

                    // Detailed WMI status comparison
                    println!("\n   Detailed WMI Changes:");

                    // PrinterStatus changes
                    if prev.printer_status_code() != current.printer_status_code() {
                        println!(
                            "   └── PrinterStatus: {:?} → {:?}",
                            prev.printer_status_code().map(|c| format!(
                                "{} ({})",
                                c,
                                prev.printer_status_description().unwrap_or("Unknown")
                            )),
                            current.printer_status_code().map(|c| format!(
                                "{} ({})",
                                c,
                                current.printer_status_description().unwrap_or("Unknown")
                            ))
                        );
                    }

                    // PrinterState changes
                    if prev.printer_state_code() != current.printer_state_code() {
                        println!(
                            "   └── PrinterState: {:?} → {:?}",
                            prev.printer_state_code().map(|c| format!(
                                "{} ({})",
                                c,
                                prev.printer_state_description().unwrap_or("Unknown")
                            )),
                            current.printer_state_code().map(|c| format!(
                                "{} ({})",
                                c,
                                current.printer_state_description().unwrap_or("Unknown")
                            ))
                        );
                    }

                    // DetectedErrorState changes
                    if prev.detected_error_state_code() != current.detected_error_state_code() {
                        println!(
                            "   └── DetectedErrorState: {:?} → {:?}",
                            prev.detected_error_state_code().map(|c| format!(
                                "{} ({})",
                                c,
                                prev.detected_error_state_description().unwrap_or("Unknown")
                            )),
                            current.detected_error_state_code().map(|c| format!(
                                "{} ({})",
                                c,
                                current
                                    .detected_error_state_description()
                                    .unwrap_or("Unknown")
                            ))
                        );
                    }

                    // ExtendedPrinterStatus changes
                    if prev.extended_printer_status_code() != current.extended_printer_status_code()
                    {
                        println!(
                            "   └── ExtendedPrinterStatus: {:?} → {:?}",
                            prev.extended_printer_status_code().map(|c| format!(
                                "{} ({})",
                                c,
                                prev.extended_printer_status_description()
                                    .unwrap_or("Unknown")
                            )),
                            current.extended_printer_status_code().map(|c| format!(
                                "{} ({})",
                                c,
                                current
                                    .extended_printer_status_description()
                                    .unwrap_or("Unknown")
                            ))
                        );
                    }

                    // WMI Status changes
                    if prev.wmi_status() != current.wmi_status() {
                        println!(
                            "   └── WMI Status: {:?} → {:?}",
                            prev.wmi_status().unwrap_or("None"),
                            current.wmi_status().unwrap_or("None")
                        );
                    }

                    println!(); // Empty line after change report
                }
            } else {
                // Initial status report
                println!("[{}] Initial Status Report", timestamp);
                println!("Printer: {}", current.name());
                println!("   Status: {}", current.status_description());
                println!("   Error State: {}", current.error_description());
                println!(
                    "   Offline: {}",
                    if current.is_offline() { "Yes" } else { "No" }
                );

                println!("\n   Current WMI Values:");
                if let Some(code) = current.printer_status_code() {
                    println!(
                        "   └── PrinterStatus: {} ({})",
                        code,
                        current.printer_status_description().unwrap_or("Unknown")
                    );
                }
                if let Some(code) = current.extended_printer_status_code() {
                    println!(
                        "   └── ExtendedPrinterStatus: {} ({})",
                        code,
                        current
                            .extended_printer_status_description()
                            .unwrap_or("Unknown")
                    );
                }
                if let Some(status) = current.wmi_status() {
                    println!("   └── WMI Status: \"{}\"", status);
                }
                println!("\n   Monitoring for changes...\n");
            }
        })
        .await?;

    Ok(())
}
