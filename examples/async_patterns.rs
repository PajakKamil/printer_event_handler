//! Advanced Async Usage Patterns Example
//!
//! This example demonstrates advanced async patterns including concurrent
//! monitoring, streaming, and background tasks with the printer library.
//!
//! Run with: cargo run --example async_patterns

use printer_event_handler::{PrinterError, PrinterMonitor};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Advanced Async Patterns");
    println!("=====================================================\n");

    // Example 1: Concurrent printer monitoring
    println!("Example 1: Concurrent Printer Monitoring");
    concurrent_monitoring().await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 2: Streaming printer status updates
    println!("Example 2: Streaming Status Updates");
    streaming_updates().await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 3: Background monitoring with channels
    println!("Example 3: Background Monitoring with Channels");
    background_monitoring().await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 4: Multiple printer concurrent analysis
    println!("Example 4: Concurrent Multi-Printer Analysis");
    concurrent_analysis().await?;

    Ok(())
}

/// Example 1: Monitor multiple printers concurrently
async fn concurrent_monitoring() -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers().await?;

    if printers.is_empty() {
        println!("   No printers found for concurrent monitoring");
        return Ok(());
    }

    println!(
        "   Starting concurrent monitoring of {} printers",
        printers.len()
    );

    // Create concurrent monitoring tasks
    let mut tasks = Vec::new();
    let monitor_arc = Arc::new(monitor);

    for printer in printers.iter().take(3) {
        // Limit to first 3 printers
        let monitor_clone = monitor_arc.clone();
        let printer_name = printer.name().to_string();

        let task = tokio::spawn({
            let monitor = monitor_clone.clone();
            async move {
                println!("   Starting monitor for: {}", printer_name);

                // Monitor this printer for a limited time
                let result = tokio::time::timeout(
                    Duration::from_secs(15),
                    monitor_single_printer(monitor, printer_name.clone()),
                )
                .await;

                match result {
                    Ok(Ok(_)) => {
                        println!("   Monitor for '{}' completed successfully", printer_name)
                    }
                    Ok(Err(e)) => println!("   Monitor for '{}' failed: {}", printer_name, e),
                    Err(_) => println!("   Monitor for '{}' timed out", printer_name),
                }
            }
        });

        tasks.push(task);
    }

    // Wait for all monitoring tasks to complete
    println!("   Waiting for all monitors to complete...");
    futures::future::join_all(tasks).await;
    println!("   All concurrent monitoring tasks completed");

    Ok(())
}

/// Helper function to monitor a single printer
async fn monitor_single_printer(
    monitor: Arc<PrinterMonitor>,
    printer_name: String,
) -> Result<(), PrinterError> {
    let mut check_count = 0;
    let mut interval = interval(Duration::from_secs(3));

    loop {
        interval.tick().await;
        check_count += 1;

        if let Some(printer) = monitor.find_printer(&printer_name).await? {
            println!(
                "   [{}] Check #{}: Status={}, WMI Status={:?}",
                printer_name,
                check_count,
                printer.status_description(),
                printer.wmi_status().unwrap_or("Unknown")
            );

            // Stop after 5 checks
            if check_count >= 5 {
                break;
            }
        } else {
            println!("   [{}] Printer not found", printer_name);
            break;
        }
    }

    Ok(())
}

/// Example 2: Stream printer status updates
async fn streaming_updates() -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers().await?;

    if printers.is_empty() {
        println!("   No printers found for streaming");
        return Ok(());
    }

    let printer_name = printers[0].name().to_string();
    println!("   Streaming status updates for: {}", printer_name);

    // Create a stream of printer status updates
    let (tx, mut rx) = mpsc::channel(100);

    // Producer task
    let producer = {
        let monitor = Arc::new(monitor);
        let printer_name = printer_name.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(2));
            let mut update_count = 0;

            loop {
                interval.tick().await;
                update_count += 1;

                if update_count > 10 {
                    // Stop after 10 updates
                    break;
                }

                match monitor.find_printer(&printer_name).await {
                    Ok(Some(printer)) => {
                        let update = PrinterStatusUpdate {
                            timestamp: chrono::Utc::now(),
                            name: printer.name().to_string(),
                            status: printer.status_description().to_string(),
                            printer_status_code: printer.printer_status_code(),
                            wmi_status: printer.wmi_status().map(String::from),
                            is_offline: printer.is_offline(),
                        };

                        if tx.send(update).await.is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Ok(None) => {
                        println!("   Printer '{}' not found", printer_name);
                        break;
                    }
                    Err(e) => {
                        println!("   Error getting printer status: {}", e);
                        break;
                    }
                }
            }

            println!("   Producer finished streaming updates");
        })
    };

    // Consumer task
    let consumer = tokio::spawn(async move {
        let mut update_count = 0;

        while let Some(update) = rx.recv().await {
            update_count += 1;
            println!(
                "   Update #{}: [{}] {} - Status: {} (Code: {:?}) - WMI: {:?} - Offline: {}",
                update_count,
                update.timestamp.format("%H:%M:%S"),
                update.name,
                update.status,
                update.printer_status_code,
                update.wmi_status.as_deref().unwrap_or("None"),
                update.is_offline
            );
        }

        println!("   Consumer finished processing updates");
    });

    // Wait for both producer and consumer to complete
    let _ = tokio::join!(producer, consumer);
    println!("   Streaming example completed");

    Ok(())
}

/// Example 3: Background monitoring with shared state
async fn background_monitoring() -> Result<(), PrinterError> {
    let monitor = Arc::new(PrinterMonitor::new().await?);
    let printers = monitor.list_printers().await?;

    if printers.is_empty() {
        println!("   No printers found for background monitoring");
        return Ok(());
    }

    // Shared state for printer statuses
    let shared_state = Arc::new(RwLock::new(HashMap::new()));

    println!("   Starting background monitoring with shared state");

    // Background monitoring task
    let background_task = {
        let monitor = monitor.clone();
        let shared_state = shared_state.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3));
            let mut iterations = 0;

            loop {
                interval.tick().await;
                iterations += 1;

                if iterations > 8 {
                    // Stop after 8 iterations
                    break;
                }

                println!("   Background scan #{}", iterations);

                match monitor.list_printers().await {
                    Ok(current_printers) => {
                        let mut state = shared_state.write().await;

                        for printer in current_printers {
                            let status_info = PrinterStatusInfo {
                                status: printer.status_description().to_string(),
                                printer_status_code: printer.printer_status_code(),
                                extended_printer_status_code: printer
                                    .extended_printer_status_code(),
                                wmi_status: printer.wmi_status().map(String::from),
                                is_offline: printer.is_offline(),
                                last_updated: chrono::Utc::now(),
                            };

                            state.insert(printer.name().to_string(), status_info);
                        }
                    }
                    Err(e) => {
                        println!("   Background scan failed: {}", e);
                    }
                }
            }

            println!("   Background monitoring task completed");
        })
    };

    // Periodic state reader task
    let reader_task = {
        let shared_state = shared_state.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            let mut reads = 0;

            loop {
                interval.tick().await;
                reads += 1;

                if reads > 5 {
                    // Stop after 5 reads
                    break;
                }

                println!("   State read #{}", reads);
                let state = shared_state.read().await;

                for (printer_name, status_info) in state.iter() {
                    println!(
                        "      {}: {} (Updated: {})",
                        printer_name,
                        status_info.status,
                        status_info.last_updated.format("%H:%M:%S")
                    );

                    if let Some(wmi_status) = &status_info.wmi_status {
                        println!("         WMI Status: {}", wmi_status);
                    }

                    if let Some(code) = status_info.printer_status_code {
                        println!("         Printer Status Code: {}", code);
                    }

                    if let Some(ext_code) = status_info.extended_printer_status_code {
                        println!("         Extended Printer Status: {}", ext_code);
                    }

                    if status_info.is_offline {
                        println!("         Status: OFFLINE");
                    }
                }
            }

            println!("   State reader task completed");
        })
    };

    // Wait for both tasks to complete
    let _ = tokio::join!(background_task, reader_task);
    println!("   Background monitoring example completed");

    Ok(())
}

/// Example 4: Concurrent analysis of multiple printers
async fn concurrent_analysis() -> Result<(), PrinterError> {
    let monitor = Arc::new(PrinterMonitor::new().await?);
    let printers = monitor.list_printers().await?;

    if printers.is_empty() {
        println!("   No printers found for analysis");
        return Ok(());
    }

    println!("   Analyzing {} printers concurrently", printers.len());

    // Create analysis tasks for each printer
    let analysis_tasks: Vec<_> = printers
        .iter()
        .map(|printer| {
            let monitor = monitor.clone();
            let printer_name = printer.name().to_string();

            tokio::spawn(async move { analyze_printer_detailed(monitor, printer_name).await })
        })
        .collect();

    // Execute all analyses concurrently and collect results
    let results = futures::future::join_all(analysis_tasks).await;

    // Process results
    let mut successful_analyses = 0;
    let mut failed_analyses = 0;

    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(Ok(analysis)) => {
                successful_analyses += 1;
                println!("   Printer #{}: {}", i + 1, analysis.summary);
                if !analysis.detailed_status.is_empty() {
                    println!("      {}", analysis.detailed_status);
                }
                println!(
                    "      Name: {}, Health Score: {}%",
                    analysis.name, analysis.health_score
                );
            }
            Ok(Err(e)) => {
                failed_analyses += 1;
                println!("   Printer #{}: Analysis failed - {}", i + 1, e);
            }
            Err(e) => {
                failed_analyses += 1;
                println!("   Printer #{}: Task failed - {}", i + 1, e);
            }
        }
    }

    println!(
        "   Analysis Summary: {} successful, {} failed",
        successful_analyses, failed_analyses
    );

    Ok(())
}

/// Detailed printer analysis
async fn analyze_printer_detailed(
    monitor: Arc<PrinterMonitor>,
    printer_name: String,
) -> Result<PrinterAnalysis, PrinterError> {
    // Simulate some analysis time
    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(printer) = monitor.find_printer(&printer_name).await? {
        let mut detailed_status = Vec::new();

        // Analyze WMI properties
        if let Some(code) = printer.printer_status_code() {
            detailed_status.push(format!("PrinterStatus: {}", code));
        }

        if let Some(code) = printer.extended_printer_status_code() {
            detailed_status.push(format!("ExtendedPrinterStatus: {}", code));
        }

        if let Some(status) = printer.wmi_status() {
            detailed_status.push(format!("WMI Status: \"{}\"", status));
        }

        let health_score = calculate_health_score(&printer);

        Ok(PrinterAnalysis {
            name: printer_name.clone(),
            summary: format!("{} - Health: {}%", printer_name, health_score),
            detailed_status: detailed_status.join(", "),
            health_score,
        })
    } else {
        Err(PrinterError::Other(format!(
            "Printer '{}' not found",
            printer_name
        )))
    }
}

/// Calculate a simple health score based on printer status
fn calculate_health_score(printer: &printer_event_handler::Printer) -> u8 {
    let mut score = 100u8;

    if printer.is_offline() {
        score = score.saturating_sub(50);
    }

    if printer.has_error() {
        score = score.saturating_sub(30);
    }

    // Check WMI status
    if let Some(wmi_status) = printer.wmi_status() {
        match wmi_status {
            "OK" => {} // No deduction
            "Degraded" => score = score.saturating_sub(20),
            "Error" => score = score.saturating_sub(40),
            _ => score = score.saturating_sub(10),
        }
    }

    score
}

/// Data structures for examples

#[derive(Debug)]
struct PrinterStatusUpdate {
    timestamp: chrono::DateTime<chrono::Utc>,
    name: String,
    status: String,
    printer_status_code: Option<u32>,
    wmi_status: Option<String>,
    is_offline: bool,
}

#[derive(Debug)]
struct PrinterStatusInfo {
    status: String,
    printer_status_code: Option<u32>,
    extended_printer_status_code: Option<u32>,
    wmi_status: Option<String>,
    is_offline: bool,
    last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
struct PrinterAnalysis {
    name: String,
    summary: String,
    detailed_status: String,
    health_score: u8,
}
