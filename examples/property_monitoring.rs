//! Property Monitoring Example
//!
//! This example demonstrates the new property-level monitoring features that can
//! detect changes in individual printer properties and provide detailed change tracking.
//!
//! Run with: cargo run --example property_monitoring

use printer_event_handler::{MonitorableProperty, PrinterError, PrinterMonitor};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Property Monitoring Example");
    println!("===================================================\n");

    let monitor = PrinterMonitor::new().await?;

    // Get printer name from command line or use first available
    let printer_name = get_target_printer_name(&monitor).await?;

    println!("Target printer: {}\n", printer_name);

    // Example 1: Detailed property change monitoring
    println!("Example 1: Detailed Property Change Monitoring");
    println!("----------------------------------------------");
    demonstrate_detailed_monitoring(&monitor, &printer_name).await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 2: Specific property monitoring
    println!("Example 2: Specific Property Monitoring");
    println!("---------------------------------------");
    demonstrate_specific_property_monitoring(&monitor, &printer_name).await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 3: Multiple printer monitoring
    println!("Example 3: Multiple Printer Monitoring");
    println!("--------------------------------------");
    demonstrate_multiple_printer_monitoring(&monitor).await?;

    Ok(())
}

/// Get the target printer name from command line or find the first available
async fn get_target_printer_name(monitor: &PrinterMonitor) -> Result<String, PrinterError> {
    if let Some(printer_name) = env::args().nth(1) {
        // Verify the printer exists
        if monitor.find_printer(&printer_name).await?.is_some() {
            return Ok(printer_name);
        } else {
            println!(
                "Warning: Printer '{}' not found, using first available printer",
                printer_name
            );
        }
    }

    // Find first available printer
    let printers = monitor.list_printers().await?;
    if printers.is_empty() {
        return Err(PrinterError::Other(
            "No printers found on this system".to_string(),
        ));
    }

    Ok(printers[0].name().to_string())
}

/// Demonstrate detailed property change monitoring
async fn demonstrate_detailed_monitoring(
    _monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!("Starting detailed monitoring for: {}", printer_name);
    println!("This will run for 30 seconds and show any property changes...");

    let printer_name_clone = printer_name.to_string();

    // Start monitoring in a background task
    let monitoring_task = {
        let printer_name = printer_name_clone.clone();
        tokio::spawn(async move {
            let new_monitor = PrinterMonitor::new().await?;
            new_monitor
                .monitor_printer_changes(&printer_name, 1000, |changes| {
                    let timestamp = changes.timestamp.format("%H:%M:%S");

                    if changes.has_changes() {
                        println!(
                            "[{}] CHANGES DETECTED for '{}': {}",
                            timestamp,
                            changes.printer_name,
                            changes.summary()
                        );

                        for change in &changes.changes {
                            println!("  - {}", change.description());
                        }
                        println!();
                    } else if changes.changes.is_empty() {
                        // This is the initial state capture
                        println!(
                            "[{}] Initial state captured for '{}'",
                            timestamp, changes.printer_name
                        );
                    }
                })
                .await
        })
    };

    // Let it run for 30 seconds
    tokio::select! {
        result = monitoring_task => {
            match result {
                Ok(Ok(_)) => println!("Monitoring completed successfully"),
                Ok(Err(e)) => println!("Monitoring failed: {}", e),
                Err(e) => println!("Monitoring task panicked: {}", e),
            }
        }
        _ = sleep(Duration::from_secs(30)) => {
            println!("Detailed monitoring example completed (30 seconds)");
        }
    }

    Ok(())
}

/// Demonstrate monitoring specific properties
async fn demonstrate_specific_property_monitoring(
    _monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!("Monitoring specific properties for: {}", printer_name);
    println!("Will monitor 'IsOffline' and 'Status' properties for 20 seconds...");

    // Monitor IsOffline property
    let printer_name1 = printer_name.to_string();
    let offline_task = {
        let printer_name = printer_name1.clone();
        tokio::spawn(async move {
            let new_monitor = PrinterMonitor::new().await?;
            new_monitor
                .monitor_property(
                    &printer_name,
                    MonitorableProperty::IsOffline,
                    1000,
                    |change| {
                        println!("OFFLINE STATUS CHANGE: {}", change.description());
                    },
                )
                .await
        })
    };

    // Monitor Status property
    let printer_name2 = printer_name.to_string();
    let status_task = {
        let printer_name = printer_name2.clone();
        tokio::spawn(async move {
            let new_monitor = PrinterMonitor::new().await?;
            new_monitor
                .monitor_property(&printer_name, MonitorableProperty::Status, 1000, |change| {
                    println!("STATUS CHANGE: {}", change.description());
                })
                .await
        })
    };

    // Let them run for 20 seconds
    tokio::select! {
        _ = offline_task => println!("Offline monitoring completed"),
        _ = status_task => println!("Status monitoring completed"),
        _ = sleep(Duration::from_secs(20)) => {
            println!("Specific property monitoring example completed (20 seconds)");
        }
    }

    Ok(())
}

/// Demonstrate monitoring multiple printers
async fn demonstrate_multiple_printer_monitoring(
    monitor: &PrinterMonitor,
) -> Result<(), PrinterError> {
    let printers = monitor.list_printers().await?;

    if printers.len() < 2 {
        println!("Need at least 2 printers for multiple printer monitoring demo");
        println!(
            "Found {} printer(s). Monitoring the first one for 15 seconds...",
            printers.len()
        );

        if !printers.is_empty() {
            let printer_names = vec![printers[0].name().to_string()];
            let monitoring_task = {
                tokio::spawn(async move {
                    let new_monitor = PrinterMonitor::new().await?;
                    new_monitor
                        .monitor_multiple_printers(printer_names, 1000, |changes| {
                            if changes.has_changes() {
                                println!(
                                    "Multi-printer monitor - {}: {}",
                                    changes.printer_name,
                                    changes.summary()
                                );
                            }
                        })
                        .await
                })
            };

            tokio::select! {
                _ = monitoring_task => println!("Multi-printer monitoring completed"),
                _ = sleep(Duration::from_secs(15)) => {
                    println!("Multiple printer monitoring example completed (15 seconds)");
                }
            }
        }
        return Ok(());
    }

    // Take up to 3 printers for demonstration
    let printer_names: Vec<String> = printers
        .iter()
        .take(3)
        .map(|p| p.name().to_string())
        .collect();

    println!("Monitoring {} printers concurrently:", printer_names.len());
    for name in &printer_names {
        println!("  - {}", name);
    }
    println!("This will run for 25 seconds...\n");

    let monitoring_task = {
        tokio::spawn(async move {
            let new_monitor = PrinterMonitor::new().await?;
            new_monitor
                .monitor_multiple_printers(printer_names, 1000, |changes| {
                    let timestamp = changes.timestamp.format("%H:%M:%S");
                    if changes.has_changes() {
                        println!(
                            "[{}] Multi-printer change - {}: {}",
                            timestamp,
                            changes.printer_name,
                            changes.summary()
                        );

                        // Show details for important changes
                        for change in &changes.changes {
                            match change.property_name() {
                                "IsOffline" | "Status" | "ErrorState" => {
                                    println!("    {}", change.description());
                                }
                                _ => {} // Skip less important properties in multi-printer mode
                            }
                        }
                    }
                })
                .await
        })
    };

    // Let it run for 25 seconds
    tokio::select! {
        result = monitoring_task => {
            match result {
                Ok(Ok(_)) => println!("Multi-printer monitoring completed successfully"),
                Ok(Err(e)) => println!("Multi-printer monitoring failed: {}", e),
                Err(e) => println!("Multi-printer monitoring task panicked: {}", e),
            }
        }
        _ = sleep(Duration::from_secs(25)) => {
            println!("Multiple printer monitoring example completed (25 seconds)");
        }
    }

    Ok(())
}
