//! Property Monitoring Example
//!
//! This example demonstrates the new property-level monitoring features that can
//! detect changes in individual printer properties and provide detailed change tracking.
//!
//! Run with: cargo run --manifest-path examples/Cargo.toml --bin property_monitoring

use printer_event_handler::{MonitorableProperty, PrinterError, PrinterMonitor};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

/// Poll cadence for every monitor spawned by this example. Tight enough that
/// you see manually-triggered changes within a second; loose enough to keep
/// WMI/CUPS load negligible.
const POLL_INTERVAL_MS: u64 = 1_000;

/// Slower cadence used by the section-header divider repeats (line of "=").
const SECTION_SEPARATOR_WIDTH: usize = 50;

/// Wall-clock budgets for each demo section. Sized so the user can flip
/// printer state manually and still see something happen before timeout.
const DETAILED_MONITOR_DURATION: Duration = Duration::from_secs(30);
const SPECIFIC_PROPERTY_DURATION: Duration = Duration::from_secs(20);
const SINGLE_PRINTER_DURATION: Duration = Duration::from_secs(15);
const MULTI_PRINTER_DURATION: Duration = Duration::from_secs(25);

/// Cap on how many printers the multi-printer demo subscribes to. Three is
/// enough to demonstrate concurrent monitoring without flooding the console.
const MAX_DEMO_PRINTERS: usize = 3;

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

    println!("\n{}\n", "=".repeat(SECTION_SEPARATOR_WIDTH));

    // Example 2: Specific property monitoring
    println!("Example 2: Specific Property Monitoring");
    println!("---------------------------------------");
    demonstrate_specific_property_monitoring(&monitor, &printer_name).await?;

    println!("\n{}\n", "=".repeat(SECTION_SEPARATOR_WIDTH));

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
        if monitor
            .find_printer_cancellable(&printer_name, None)
            .await?
            .is_some()
        {
            return Ok(printer_name);
        } else {
            println!(
                "Warning: Printer '{}' not found, using first available printer",
                printer_name
            );
        }
    }

    // Find first available printer
    let printers = monitor.list_printers_cancellable(None).await?;
    if printers.is_empty() {
        return Err(PrinterError::Other(
            "No printers found on this system".to_string(),
        ));
    }

    Ok(printers[0].name().to_string())
}

/// Demonstrate detailed property change monitoring
async fn demonstrate_detailed_monitoring(
    monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!("Starting detailed monitoring for: {}", printer_name);
    println!(
        "This will run for {} seconds and show any property changes...",
        DETAILED_MONITOR_DURATION.as_secs()
    );

    // Start monitoring in a background task - clone the existing monitor
    // (cheap Arc clone, shares the same backend) rather than re-initialising.
    // We drive it via the 2.0 `MonitorBuilder::run_changes` so this example
    // doubles as a demo of the recommended fluent API.
    let monitoring_task = {
        let printer_name = printer_name.to_string();
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor(&printer_name)
                .interval_ms(POLL_INTERVAL_MS)
                .run_changes(|changes| {
                    // `run_changes` only fires when properties actually
                    // mutated; the initial snapshot is captured silently by
                    // the monitor (see PrinterMonitor::monitor_printer_changes
                    // docs). So `changes` always carries at least one entry
                    // here - no `has_changes()` guard needed.
                    let timestamp = changes.timestamp.format("%H:%M:%S");
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
                })
                .await
        })
    };

    tokio::select! {
        result = monitoring_task => {
            match result {
                Ok(Ok(_)) => println!("Monitoring completed successfully"),
                Ok(Err(e)) => println!("Monitoring failed: {}", e),
                Err(e) => println!("Monitoring task panicked: {}", e),
            }
        }
        _ = sleep(DETAILED_MONITOR_DURATION) => {
            println!(
                "Detailed monitoring example completed ({} seconds)",
                DETAILED_MONITOR_DURATION.as_secs()
            );
        }
    }

    Ok(())
}

/// Demonstrate monitoring specific properties
async fn demonstrate_specific_property_monitoring(
    monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!("Monitoring specific properties for: {}", printer_name);
    println!(
        "Will monitor 'IsOffline' and 'Status' properties for {} seconds...",
        SPECIFIC_PROPERTY_DURATION.as_secs()
    );

    // Monitor IsOffline property via `MonitorBuilder::filter_property`.
    let offline_task = {
        let printer_name = printer_name.to_string();
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor(&printer_name)
                .interval_ms(POLL_INTERVAL_MS)
                .filter_property(MonitorableProperty::IsOffline)
                .run_property(|change| {
                    println!("OFFLINE STATUS CHANGE: {}", change.description());
                })
                .await
        })
    };

    // Monitor Status property - same builder, different filter.
    let status_task = {
        let printer_name = printer_name.to_string();
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor(&printer_name)
                .interval_ms(POLL_INTERVAL_MS)
                .filter_property(MonitorableProperty::Status)
                .run_property(|change| {
                    println!("STATUS CHANGE: {}", change.description());
                })
                .await
        })
    };

    tokio::select! {
        _ = offline_task => println!("Offline monitoring completed"),
        _ = status_task => println!("Status monitoring completed"),
        _ = sleep(SPECIFIC_PROPERTY_DURATION) => {
            println!(
                "Specific property monitoring example completed ({} seconds)",
                SPECIFIC_PROPERTY_DURATION.as_secs()
            );
        }
    }

    Ok(())
}

/// Demonstrate monitoring multiple printers
async fn demonstrate_multiple_printer_monitoring(
    monitor: &PrinterMonitor,
) -> Result<(), PrinterError> {
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.len() < 2 {
        println!("Need at least 2 printers for multiple printer monitoring demo");
        println!(
            "Found {} printer(s). Monitoring the first one for {} seconds...",
            printers.len(),
            SINGLE_PRINTER_DURATION.as_secs()
        );

        if !printers.is_empty() {
            let printer_names = vec![printers[0].name().to_string()];
            let monitoring_task = {
                let monitor = monitor.clone();
                tokio::spawn(async move {
                    monitor
                        .monitor_multiple_printers(
                            printer_names,
                            POLL_INTERVAL_MS,
                            None,
                            |changes| {
                                if changes.has_changes() {
                                    println!(
                                        "Multi-printer monitor - {}: {}",
                                        changes.printer_name,
                                        changes.summary()
                                    );
                                }
                            },
                        )
                        .await
                })
            };

            tokio::select! {
                _ = monitoring_task => println!("Multi-printer monitoring completed"),
                _ = sleep(SINGLE_PRINTER_DURATION) => {
                    println!(
                        "Multiple printer monitoring example completed ({} seconds)",
                        SINGLE_PRINTER_DURATION.as_secs()
                    );
                }
            }
        }
        return Ok(());
    }

    // Take up to MAX_DEMO_PRINTERS for demonstration.
    let printer_names: Vec<String> = printers
        .iter()
        .take(MAX_DEMO_PRINTERS)
        .map(|p| p.name().to_string())
        .collect();

    println!("Monitoring {} printers concurrently:", printer_names.len());
    for name in &printer_names {
        println!("  - {}", name);
    }
    println!(
        "This will run for {} seconds...\n",
        MULTI_PRINTER_DURATION.as_secs()
    );

    let monitoring_task = {
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor_multiple_printers(printer_names, POLL_INTERVAL_MS, None, |changes| {
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

    tokio::select! {
        result = monitoring_task => {
            match result {
                Ok(Ok(_)) => println!("Multi-printer monitoring completed successfully"),
                Ok(Err(e)) => println!("Multi-printer monitoring failed: {}", e),
                Err(e) => println!("Multi-printer monitoring task panicked: {}", e),
            }
        }
        _ = sleep(MULTI_PRINTER_DURATION) => {
            println!(
                "Multiple printer monitoring example completed ({} seconds)",
                MULTI_PRINTER_DURATION.as_secs()
            );
        }
    }

    Ok(())
}
