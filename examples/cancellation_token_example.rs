//! Cancellation Token Example
//!
//! This example demonstrates how to use cancellation tokens to gracefully stop
//! printer monitoring tasks. This is useful for implementing clean shutdown,
//! responsive UI controls, or conditional monitoring based on application state.
//!
//! Run with: cargo run --bin cancellation_token_example

use printer_event_handler::{MonitorableProperty, PrinterError, PrinterMonitor};
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Cancellation Token Examples");
    println!("===================================================\n");

    let monitor = PrinterMonitor::new().await?;

    // Get printer name from command line or use first available
    let printer_name = get_target_printer_name(&monitor).await?;

    println!("Target printer: {}\n", printer_name);

    // Example 1: Basic cancellation after timeout
    println!("Example 1: Basic Cancellation After Timeout");
    println!("-------------------------------------------");
    basic_cancellation_example(&monitor, &printer_name).await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 2: Multiple monitors with individual cancellation
    println!("Example 2: Multiple Monitors with Individual Cancellation");
    println!("---------------------------------------------------------");
    multiple_monitors_cancellation(&monitor).await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 3: Coordinated shutdown of multiple monitors
    println!("Example 3: Coordinated Shutdown");
    println!("-------------------------------");
    coordinated_shutdown(&monitor, &printer_name).await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 4: Conditional cancellation based on printer state
    println!("Example 4: Conditional Cancellation");
    println!("-----------------------------------");
    conditional_cancellation(&monitor, &printer_name).await?;

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

/// Example 1: Basic cancellation - start monitoring and cancel after a timeout
async fn basic_cancellation_example(
    _monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!("Starting monitoring for 10 seconds, then cancelling...");

    // Create a cancellation token
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    // Spawn monitoring task
    let printer_name_owned = printer_name.to_string();
    let monitor_task = {
        let monitor = PrinterMonitor::new().await?;
        tokio::spawn(async move {
            monitor
                .monitor_printer_changes(
                    &printer_name_owned,
                    1000,
                    Some(cancel_token_clone),
                    |changes| {
                        let timestamp = changes.timestamp.format("%H:%M:%S");
                        if changes.has_changes() {
                            println!("[{}] Changes detected: {}", timestamp, changes.summary());
                        } else {
                            println!("[{}] Monitoring active...", timestamp);
                        }
                    },
                )
                .await
        })
    };

    // Wait for 10 seconds
    println!("Waiting 10 seconds before cancellation...");
    sleep(Duration::from_secs(10)).await;

    // Cancel the monitoring
    println!("Cancelling monitoring...");
    cancel_token.cancel();

    // Wait for the monitoring task to complete
    match monitor_task.await {
        Ok(Ok(())) => println!("Monitoring stopped gracefully"),
        Ok(Err(e)) => println!("Monitoring stopped with error: {}", e),
        Err(e) => println!("Task join error: {}", e),
    }

    Ok(())
}

/// Example 2: Multiple monitors with individual cancellation
async fn multiple_monitors_cancellation(_monitor: &PrinterMonitor) -> Result<(), PrinterError> {
    let printers = _monitor.list_printers().await?;

    if printers.is_empty() {
        println!("No printers found for this example");
        return Ok(());
    }

    println!(
        "Starting {} monitor(s) with different cancellation times...",
        printers.len().min(3)
    );

    // Create monitoring tasks with different cancellation times
    let mut tasks = Vec::new();
    let mut cancel_tokens = Vec::new();

    for (i, printer) in printers.iter().take(3).enumerate() {
        let cancel_token = CancellationToken::new();
        let printer_name = printer.name().to_string();
        let cancel_duration = Duration::from_secs(5 + (i as u64 * 3));

        println!(
            "   Monitor #{}: {} (will cancel after {} seconds)",
            i + 1,
            printer_name,
            cancel_duration.as_secs()
        );

        // Spawn monitoring task
        let cancel_clone = cancel_token.clone();
        let task = {
            let printer_name_clone = printer_name.clone();
            tokio::spawn(async move {
                let new_monitor = PrinterMonitor::new().await?;
                new_monitor
                    .monitor_property(
                        &printer_name_clone,
                        MonitorableProperty::Status,
                        2000,
                        Some(cancel_clone),
                        move |change| {
                            println!("   [Monitor #{}] {}", i + 1, change.description());
                        },
                    )
                    .await
            })
        };

        // Spawn cancellation task
        let cancel_token_for_cancel = cancel_token.clone();
        tokio::spawn(async move {
            sleep(cancel_duration).await;
            println!("   Cancelling Monitor #{} for '{}'", i + 1, printer_name);
            cancel_token_for_cancel.cancel();
        });

        tasks.push(task);
        cancel_tokens.push(cancel_token);
    }

    // Wait for all tasks to complete
    println!("\nWaiting for all monitors to be cancelled...");
    for (i, task) in tasks.into_iter().enumerate() {
        match task.await {
            Ok(Ok(())) => println!("   Monitor #{} stopped", i + 1),
            Ok(Err(e)) => println!("   Monitor #{} error: {}", i + 1, e),
            Err(e) => println!("   Monitor #{} join error: {}", i + 1, e),
        }
    }

    println!("All monitors have been cancelled");

    Ok(())
}

/// Example 3: Coordinated shutdown - cancel all monitors at once
async fn coordinated_shutdown(
    _monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!(
        "Starting multiple monitoring types for '{}', all sharing one cancellation token...",
        printer_name
    );

    // Single cancellation token for all monitors
    let cancel_token = CancellationToken::new();

    // Start multiple monitoring tasks with the same cancellation token
    let printer_name_owned = printer_name.to_string();

    // Monitor 1: General changes
    let task1 = {
        let cancel = cancel_token.clone();
        let name = printer_name_owned.clone();
        tokio::spawn(async move {
            let monitor = PrinterMonitor::new().await?;
            monitor
                .monitor_printer_changes(&name, 2000, Some(cancel), |changes| {
                    if changes.has_changes() {
                        println!("   [Changes Monitor] {}", changes.summary());
                    }
                })
                .await
        })
    };

    // Monitor 2: Offline status
    let task2 = {
        let cancel = cancel_token.clone();
        let name = printer_name_owned.clone();
        tokio::spawn(async move {
            let monitor = PrinterMonitor::new().await?;
            monitor
                .monitor_property(
                    &name,
                    MonitorableProperty::IsOffline,
                    2000,
                    Some(cancel),
                    |change| {
                        println!("   [Offline Monitor] {}", change.description());
                    },
                )
                .await
        })
    };

    // Monitor 3: Status changes
    let task3 = {
        let cancel = cancel_token.clone();
        let name = printer_name_owned.clone();
        tokio::spawn(async move {
            let monitor = PrinterMonitor::new().await?;
            monitor
                .monitor_property(
                    &name,
                    MonitorableProperty::Status,
                    2000,
                    Some(cancel),
                    |change| {
                        println!("   [Status Monitor] {}", change.description());
                    },
                )
                .await
        })
    };

    println!("Three monitors started. Will cancel all in 8 seconds...");
    sleep(Duration::from_secs(8)).await;

    // Cancel all monitors at once
    println!("\nCancelling all monitors...");
    cancel_token.cancel();

    // Wait for all tasks to complete
    let results = tokio::join!(task1, task2, task3);

    println!("All monitors stopped:");
    println!("   Changes Monitor: {:?}", results.0.is_ok());
    println!("   Offline Monitor: {:?}", results.1.is_ok());
    println!("   Status Monitor: {:?}", results.2.is_ok());

    Ok(())
}

/// Example 4: Conditional cancellation - cancel based on printer state
async fn conditional_cancellation(
    _monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!(
        "Monitoring '{}' and will auto-cancel if printer goes offline or has an error...",
        printer_name
    );
    println!("(Or after 15 seconds, whichever comes first)");

    let cancel_token = CancellationToken::new();
    let cancel_for_monitor = cancel_token.clone();
    let cancel_for_timeout = cancel_token.clone();

    // Start monitoring
    let printer_name_owned = printer_name.to_string();
    let monitor_task = {
        let name = printer_name_owned.clone();
        tokio::spawn(async move {
            let monitor = PrinterMonitor::new().await?;
            monitor
                .monitor_printer_changes(&name, 1000, Some(cancel_for_monitor), |changes| {
                    let timestamp = changes.timestamp.format("%H:%M:%S");
                    println!("[{}] Monitoring active...", timestamp);

                    if changes.has_changes() {
                        println!("   Changes: {}", changes.summary());
                    }
                })
                .await
        })
    };

    // Check printer state periodically and cancel if offline or error
    let state_check_task = {
        let name = printer_name_owned.clone();
        let cancel = cancel_token.clone();
        tokio::spawn(async move {
            let monitor = PrinterMonitor::new().await?;
            let mut interval = tokio::time::interval(Duration::from_secs(2));

            loop {
                interval.tick().await;

                if let Some(printer) = monitor.find_printer(&name).await? {
                    if printer.is_offline() {
                        println!("\nPrinter went offline! Cancelling monitoring...");
                        cancel.cancel();
                        break;
                    }

                    if printer.has_error() {
                        println!("\nPrinter has error! Cancelling monitoring...");
                        cancel.cancel();
                        break;
                    }
                }

                // Stop if already cancelled
                if cancel.is_cancelled() {
                    break;
                }
            }

            Ok::<(), PrinterError>(())
        })
    };

    // Timeout after 15 seconds
    let timeout_task = tokio::spawn(async move {
        sleep(Duration::from_secs(15)).await;
        if !cancel_for_timeout.is_cancelled() {
            println!("\nTimeout reached (15 seconds). Cancelling monitoring...");
            cancel_for_timeout.cancel();
        }
    });

    // Wait for monitoring to complete
    match monitor_task.await {
        Ok(Ok(())) => println!("Monitoring stopped gracefully"),
        Ok(Err(e)) => println!("Monitoring error: {}", e),
        Err(e) => println!("Task join error: {}", e),
    }

    // Clean up other tasks
    let _ = state_check_task.await;
    let _ = timeout_task.await;

    println!("Conditional cancellation example complete");

    Ok(())
}
