//! Advanced Async Usage Patterns Example
//!
//! Demonstrates idiomatic Tokio + library patterns:
//! - `JoinSet` for fanned-out concurrent tasks
//! - `CancellationToken` plumbed through `tokio::select!` for responsive cancellation
//! - `tokio::signal::ctrl_c` handler that cleanly stops every running example
//! - `mpsc` + `ReceiverStream` for back-pressured streaming
//! - `RwLock`-backed shared state
//! - The library's own `PrinterMonitor::monitor_multiple_printers` as the
//!   high-level equivalent of the hand-rolled fan-out in Example 1
//!
//! See also:
//! - `cancellation_token_example.rs` - focused walkthrough of `CancellationToken`
//! - `property_monitoring.rs` - fine-grained `MonitorableProperty` usage
//! - `monitor_changes.rs` - basic change detection with `monitor_printer_changes`
//!
//! Run with:
//! ```bash
//! cargo run --manifest-path examples/Cargo.toml --bin async_patterns
//! ```

use printer_event_handler::{CancellationToken, Printer, PrinterError, PrinterMonitor};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinSet;
use tokio::time::interval;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

// Tuning constants - kept here so each example body reads as intent, not numbers.
const MAX_CONCURRENT_MONITORS: usize = 3;
const MONITOR_TICK: Duration = Duration::from_secs(3);
const MONITOR_CHECKS: usize = 5;

const STREAM_TICK: Duration = Duration::from_secs(2);
const STREAM_UPDATES: usize = 10;
// Intentionally small: the producer awaits `tx.send`, so when the buffer is
// full it stalls until the consumer drains an item. With a fast consumer the
// buffer stays nearly empty; with a slow consumer the producer pauses
// naturally. A buffer of 4 is enough to absorb short jitter without masking
// real back-pressure behaviour.
const STREAM_BUFFER: usize = 4;

const BACKGROUND_SCAN_TICK: Duration = Duration::from_secs(3);
const BACKGROUND_SCAN_ITERATIONS: usize = 8;
const STATE_READ_TICK: Duration = Duration::from_secs(5);
const STATE_READS: usize = 5;

const LIBRARY_MONITOR_INTERVAL_MS: u64 = 2_000;
const LIBRARY_MONITOR_DURATION: Duration = Duration::from_secs(10);

const SECTION_SEPARATOR_WIDTH: usize = 50;

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Advanced Async Patterns");
    println!("=====================================================");
    println!("Press Ctrl+C at any time to shut down gracefully.\n");

    // One master CancellationToken cancels every running example when Ctrl+C
    // is pressed. Each example also clones it for its own internal tasks.
    let shutdown = CancellationToken::new();
    spawn_ctrl_c_handler(shutdown.clone());

    run_example(
        "Example 1: Concurrent Printer Monitoring (hand-rolled with JoinSet)",
        &shutdown,
        concurrent_monitoring(shutdown.clone()),
    )
    .await?;

    run_example(
        "Example 2: Streaming Status Updates (mpsc + ReceiverStream)",
        &shutdown,
        streaming_updates(shutdown.clone()),
    )
    .await?;

    run_example(
        "Example 3: Background Monitoring with Shared State",
        &shutdown,
        background_monitoring(shutdown.clone()),
    )
    .await?;

    run_example(
        "Example 4: Concurrent Multi-Printer Analysis",
        &shutdown,
        concurrent_analysis(shutdown.clone()),
    )
    .await?;

    run_example(
        "Example 5: Library-driven monitor_multiple_printers (high-level API)",
        &shutdown,
        library_multi_monitor(shutdown.clone()),
    )
    .await?;

    if shutdown.is_cancelled() {
        println!("Shutdown complete.");
    } else {
        println!("All examples finished.");
    }
    Ok(())
}

fn spawn_ctrl_c_handler(shutdown: CancellationToken) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            println!("\n   [Ctrl+C received - cancelling all running examples...]");
            shutdown.cancel();
        }
    });
}

async fn run_example<F>(
    label: &str,
    shutdown: &CancellationToken,
    fut: F,
) -> Result<(), PrinterError>
where
    F: std::future::Future<Output = Result<(), PrinterError>>,
{
    if shutdown.is_cancelled() {
        return Ok(());
    }
    println!("{}", label);
    fut.await?;
    println!("\n{}\n", "-".repeat(SECTION_SEPARATOR_WIDTH));
    Ok(())
}

/// Example 1: Monitor multiple printers concurrently using `JoinSet` and a
/// shared `CancellationToken`. This is the hand-rolled version - see
/// Example 5 for the library's built-in equivalent.
async fn concurrent_monitoring(shutdown: CancellationToken) -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.is_empty() {
        println!("   No printers found for concurrent monitoring");
        return Ok(());
    }

    let take_count = printers.len().min(MAX_CONCURRENT_MONITORS);
    println!("   Starting concurrent monitoring of {} printers", take_count);

    let mut tasks = JoinSet::new();

    for printer in printers.iter().take(MAX_CONCURRENT_MONITORS) {
        let monitor = monitor.clone(); // cheap Arc clone
        let printer_name = printer.name().to_string();
        let cancel = shutdown.clone();

        tasks.spawn(async move {
            println!("   Starting monitor for: {}", printer_name);

            match monitor_single_printer(monitor, printer_name.clone(), cancel).await {
                Ok(_) => println!("   Monitor for '{}' completed successfully", printer_name),
                Err(e) => println!("   Monitor for '{}' failed: {}", printer_name, e),
            }
        });
    }

    println!("   Waiting for all monitors to complete...");
    while tasks.join_next().await.is_some() {}
    println!("   All concurrent monitoring tasks completed");

    Ok(())
}

/// Helper: monitor one printer until `MONITOR_CHECKS` ticks have elapsed or
/// the cancellation token fires. `PrinterMonitor` is taken by value because
/// it is cheaply `Clone`able (wraps `Arc<dyn PrinterBackend>` internally).
async fn monitor_single_printer(
    monitor: PrinterMonitor,
    printer_name: String,
    cancel: CancellationToken,
) -> Result<(), PrinterError> {
    let mut check_count = 0;
    let mut ticker = interval(MONITOR_TICK);

    loop {
        // Two-way select: tick or cancel. Whichever fires first wins; the
        // loser future is dropped cleanly.
        tokio::select! {
            _ = ticker.tick() => {}
            _ = cancel.cancelled() => return Ok(()),
        }
        check_count += 1;

        if let Some(printer) = monitor
            .find_printer_cancellable(&printer_name, None)
            .await?
        {
            println!(
                "   [{}] Check #{}: Status={}, WMI Status={:?}",
                printer_name,
                check_count,
                printer.status_description(),
                printer.wmi_status().unwrap_or("Unknown")
            );

            if check_count >= MONITOR_CHECKS {
                break;
            }
        } else {
            return Err(PrinterError::PrinterNotFound(printer_name));
        }
    }

    Ok(())
}

/// Example 2: Stream printer status updates through an mpsc channel exposed
/// as a `Stream` via `ReceiverStream`. The buffer is intentionally small to
/// demonstrate back-pressure (see `STREAM_BUFFER` comment).
async fn streaming_updates(shutdown: CancellationToken) -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.is_empty() {
        println!("   No printers found for streaming");
        return Ok(());
    }

    let printer_name = printers[0].name().to_string();
    println!(
        "   Streaming status updates for: {} (channel buffer = {})",
        printer_name, STREAM_BUFFER
    );

    let (tx, rx) = mpsc::channel::<PrinterStatusUpdate>(STREAM_BUFFER);
    let mut stream = ReceiverStream::new(rx);

    let producer = {
        let monitor = monitor.clone();
        let printer_name = printer_name.clone();
        let cancel = shutdown.clone();
        tokio::spawn(async move {
            let mut ticker = interval(STREAM_TICK);
            let mut update_count = 0;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {}
                    _ = cancel.cancelled() => break,
                }
                update_count += 1;

                if update_count > STREAM_UPDATES {
                    break;
                }

                match monitor
                    .find_printer_cancellable(&printer_name, None)
                    .await
                {
                    Ok(Some(printer)) => {
                        let update = PrinterStatusUpdate {
                            timestamp: chrono::Local::now(),
                            name: printer.name().to_string(),
                            status: printer.status_description().to_string(),
                            printer_status_code: printer.printer_status_code(),
                            wmi_status: printer.wmi_status().map(String::from),
                            is_offline: printer.is_offline(),
                        };

                        // `tx.send` awaits when the buffer is full; this is
                        // the back-pressure point for the producer.
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

    // Consumer driven as a Stream rather than `rx.recv()` in a while-let.
    // Demonstrates StreamExt adapters (take, map, filter, ...) - here just
    // iterating, but the pattern unlocks the full streams toolkit.
    let mut update_count = 0;
    while let Some(update) = stream.next().await {
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

    let _ = producer.await;
    println!("   Streaming example completed");
    Ok(())
}

/// Example 3: A background scanner writes printer state into a shared
/// `RwLock<HashMap>`, while a reader periodically reports the snapshot.
async fn background_monitoring(shutdown: CancellationToken) -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.is_empty() {
        println!("   No printers found for background monitoring");
        return Ok(());
    }

    let shared_state = Arc::new(RwLock::new(HashMap::new()));

    println!("   Starting background monitoring with shared state");

    let background_task = {
        let monitor = monitor.clone();
        let shared_state = shared_state.clone();
        let cancel = shutdown.clone();
        tokio::spawn(async move {
            let mut ticker = interval(BACKGROUND_SCAN_TICK);
            let mut iterations = 0;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {}
                    _ = cancel.cancelled() => break,
                }
                iterations += 1;

                if iterations > BACKGROUND_SCAN_ITERATIONS {
                    break;
                }

                println!("   Background scan #{}", iterations);

                match monitor.list_printers_cancellable(None).await {
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
                                last_updated: chrono::Local::now(),
                            };

                            state.insert(printer.name().to_string(), status_info);
                        }
                    }
                    Err(e) => println!("   Background scan failed: {}", e),
                }
            }

            println!("   Background monitoring task completed");
        })
    };

    let reader_task = {
        let shared_state = shared_state.clone();
        let cancel = shutdown.clone();
        tokio::spawn(async move {
            let mut ticker = interval(STATE_READ_TICK);
            let mut reads = 0;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {}
                    _ = cancel.cancelled() => break,
                }
                reads += 1;

                if reads > STATE_READS {
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

    let _ = tokio::join!(background_task, reader_task);
    println!("   Background monitoring example completed");

    Ok(())
}

/// Example 4: Analyse every printer concurrently. Uses the `Printer` objects
/// already returned by `list_printers()` - no extra WMI/CUPS queries.
async fn concurrent_analysis(shutdown: CancellationToken) -> Result<(), PrinterError> {
    if shutdown.is_cancelled() {
        return Ok(());
    }

    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.is_empty() {
        println!("   No printers found for analysis");
        return Ok(());
    }

    println!("   Analyzing {} printers concurrently", printers.len());

    let mut tasks: JoinSet<PrinterAnalysis> = JoinSet::new();
    for printer in printers {
        tasks.spawn(async move { analyze_printer_detailed(printer) });
    }

    let mut index = 0;
    let mut successful_analyses = 0;
    let mut failed_analyses = 0;

    while let Some(result) = tasks.join_next().await {
        index += 1;
        match result {
            Ok(analysis) => {
                successful_analyses += 1;
                println!("   Printer #{}: {}", index, analysis.summary);
                if !analysis.detailed_status.is_empty() {
                    println!("      {}", analysis.detailed_status);
                }
                println!("      Health Score: {}%", analysis.health_score);
            }
            Err(e) => {
                failed_analyses += 1;
                println!("   Printer #{}: Task failed - {}", index, e);
            }
        }
    }

    println!(
        "   Analysis Summary: {} successful, {} failed",
        successful_analyses, failed_analyses
    );

    Ok(())
}

/// Example 5: The library's `monitor_multiple_printers` is the high-level
/// equivalent of Example 1 - one call, one shared backend, one cancellation
/// token. The local token below derives from the master `shutdown` token but
/// can also be cancelled by our duration limit, so the example finishes
/// without requiring Ctrl+C.
async fn library_multi_monitor(shutdown: CancellationToken) -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.is_empty() {
        println!("   No printers found for library-driven monitoring");
        return Ok(());
    }

    let names: Vec<String> = printers
        .iter()
        .take(MAX_CONCURRENT_MONITORS)
        .map(|p| p.name().to_string())
        .collect();

    println!(
        "   Library-driven monitoring of {} printers for {:?}",
        names.len(),
        LIBRARY_MONITOR_DURATION
    );

    // Local cancellation: fires from either the master shutdown token or our
    // duration limit, whichever comes first. Three-way coordination without
    // spawning a CancellationToken hierarchy.
    let local = CancellationToken::new();
    {
        let local = local.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(LIBRARY_MONITOR_DURATION) => {
                    println!("   Time limit reached - cancelling library monitor");
                }
                _ = shutdown.cancelled() => {
                    println!("   Master shutdown received - cancelling library monitor");
                }
            }
            local.cancel();
        });
    }

    monitor
        .monitor_multiple_printers(names, LIBRARY_MONITOR_INTERVAL_MS, Some(local), |changes| {
            println!(
                "   [{}] {} change(s): {}",
                changes.printer_name,
                changes.change_count(),
                changes.summary()
            );
            for change in &changes.changes {
                println!("      - {}", change.description());
            }
        })
        .await?;

    println!("   Library-driven monitor completed");
    Ok(())
}

fn analyze_printer_detailed(printer: Printer) -> PrinterAnalysis {
    let mut detailed_status = Vec::new();

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

    PrinterAnalysis {
        summary: format!("{} - Health: {}%", printer.name(), health_score),
        detailed_status: detailed_status.join(", "),
        health_score,
    }
}

// Health-score penalty values - kept as named constants so the function body
// reads as intent ("subtract OFFLINE_PENALTY") rather than magic numbers.
const FULL_HEALTH: u8 = 100;
const OFFLINE_PENALTY: u8 = 50;
const ERROR_PENALTY: u8 = 30;
const WMI_DEGRADED_PENALTY: u8 = 20;
const WMI_ERROR_PENALTY: u8 = 40;
const WMI_UNKNOWN_PENALTY: u8 = 10;

fn calculate_health_score(printer: &Printer) -> u8 {
    let mut score = FULL_HEALTH;

    if printer.is_offline() {
        score = score.saturating_sub(OFFLINE_PENALTY);
    }

    if printer.has_error() {
        score = score.saturating_sub(ERROR_PENALTY);
    }

    if let Some(wmi_status) = printer.wmi_status() {
        match wmi_status {
            "OK" => {}
            "Degraded" => score = score.saturating_sub(WMI_DEGRADED_PENALTY),
            "Error" => score = score.saturating_sub(WMI_ERROR_PENALTY),
            _ => score = score.saturating_sub(WMI_UNKNOWN_PENALTY),
        }
    }

    score
}

#[derive(Debug)]
struct PrinterStatusUpdate {
    timestamp: chrono::DateTime<chrono::Local>,
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
    last_updated: chrono::DateTime<chrono::Local>,
}

#[derive(Debug)]
struct PrinterAnalysis {
    summary: String,
    detailed_status: String,
    health_score: u8,
}
