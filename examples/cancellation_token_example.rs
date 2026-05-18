//! Cancellation Token Example
//!
//! This example demonstrates how to use cancellation tokens to gracefully stop
//! printer monitoring tasks. This is useful for implementing clean shutdown,
//! responsive UI controls, or conditional monitoring based on application state.
//!
//! Run with: cargo run --manifest-path examples/Cargo.toml --bin cancellation_token_example

use printer_event_handler::{MonitorableProperty, PrinterError, PrinterMonitor};
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

/// Section separator length (in `=` characters) used between examples. Wide
/// enough that the section break is visible but not so wide it wraps in a
/// typical terminal.
const SECTION_SEPARATOR_WIDTH: usize = 50;

/// Poll cadence used by the basic / coordinated examples. Tight cadence so the
/// example finishes its 8-15 second windows with meaningful polling activity.
const FAST_POLL_INTERVAL_MS: u64 = 1_000;
/// Slightly looser cadence for the multi-monitor example; the staggered
/// cancellation timings are easier to follow when each monitor logs less.
const SLOW_POLL_INTERVAL_MS: u64 = 2_000;

/// Wall-clock budgets per example section.
const BASIC_CANCEL_AFTER: Duration = Duration::from_secs(10);
/// Base cancellation delay for the multi-monitor example; each successive
/// monitor gets an additional `MULTI_MONITOR_STAGGER` on top so they cancel at
/// different times.
const MULTI_MONITOR_BASE_DELAY_SECS: u64 = 5;
const MULTI_MONITOR_STAGGER_SECS: u64 = 3;
const COORDINATED_CANCEL_AFTER: Duration = Duration::from_secs(8);
const CONDITIONAL_TIMEOUT: Duration = Duration::from_secs(15);
/// Cadence at which `conditional_cancellation` rechecks printer state to
/// decide whether to fire the cancel token early.
const CONDITIONAL_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Max number of printers monitored simultaneously in
/// `multiple_monitors_cancellation`.
const MAX_MULTI_MONITORS: usize = 3;

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

    println!("\n{}\n", "=".repeat(SECTION_SEPARATOR_WIDTH));

    // Example 2: Multiple monitors with individual cancellation
    println!("Example 2: Multiple Monitors with Individual Cancellation");
    println!("---------------------------------------------------------");
    multiple_monitors_cancellation(&monitor).await?;

    println!("\n{}\n", "=".repeat(SECTION_SEPARATOR_WIDTH));

    // Example 3: Coordinated shutdown of multiple monitors
    println!("Example 3: Coordinated Shutdown");
    println!("-------------------------------");
    coordinated_shutdown(&monitor, &printer_name).await?;

    println!("\n{}\n", "=".repeat(SECTION_SEPARATOR_WIDTH));

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

/// Example 1: Basic cancellation - start monitoring and cancel after a timeout
async fn basic_cancellation_example(
    monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!(
        "Starting monitoring for {} seconds, then cancelling...",
        BASIC_CANCEL_AFTER.as_secs()
    );

    // Create a cancellation token
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    // Spawn monitoring task - clone the existing monitor (cheap Arc clone).
    // This is the recommended 2.0 form: `MonitorBuilder::cancel_token`
    // attaches the token via the builder, no positional `Some(...)` plumbing.
    let printer_name_owned = printer_name.to_string();
    let monitor_task = {
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor(&printer_name_owned)
                .interval_ms(FAST_POLL_INTERVAL_MS)
                .cancel_token(cancel_token_clone)
                .run_changes(|changes| {
                    let timestamp = changes.timestamp.format("%H:%M:%S");
                    // `run_changes` only fires when properties actually
                    // changed - the empty-changes branch is unreachable.
                    println!("[{}] Changes detected: {}", timestamp, changes.summary());
                })
                .await
        })
    };

    println!(
        "Waiting {} seconds before cancellation...",
        BASIC_CANCEL_AFTER.as_secs()
    );
    sleep(BASIC_CANCEL_AFTER).await;

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
async fn multiple_monitors_cancellation(monitor: &PrinterMonitor) -> Result<(), PrinterError> {
    let printers = monitor.list_printers_cancellable(None).await?;

    if printers.is_empty() {
        println!("No printers found for this example");
        return Ok(());
    }

    println!(
        "Starting {} monitor(s) with different cancellation times...",
        printers.len().min(MAX_MULTI_MONITORS)
    );

    // Create monitoring tasks with different cancellation times
    let mut tasks = Vec::new();
    let mut cancel_tokens = Vec::new();

    for (i, printer) in printers.iter().take(MAX_MULTI_MONITORS).enumerate() {
        let cancel_token = CancellationToken::new();
        let printer_name = printer.name().to_string();
        let cancel_duration = Duration::from_secs(
            MULTI_MONITOR_BASE_DELAY_SECS + (i as u64 * MULTI_MONITOR_STAGGER_SECS),
        );

        println!(
            "   Monitor #{}: {} (will cancel after {} seconds)",
            i + 1,
            printer_name,
            cancel_duration.as_secs()
        );

        // Spawn monitoring task - clone the existing monitor (Arc clone).
        let cancel_clone = cancel_token.clone();
        let task = {
            let printer_name_clone = printer_name.clone();
            let monitor = monitor.clone();
            tokio::spawn(async move {
                monitor
                    .monitor_property(
                        &printer_name_clone,
                        MonitorableProperty::Status,
                        SLOW_POLL_INTERVAL_MS,
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
    monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!(
        "Starting multiple monitoring types for '{}', all sharing one cancellation token...",
        printer_name
    );

    // Single cancellation token for all monitors
    let cancel_token = CancellationToken::new();

    // Start multiple monitoring tasks with the same cancellation token.
    // All three tasks share the same backend via cheap Arc clones.
    let printer_name_owned = printer_name.to_string();

    // Monitor 1: General changes
    let task1 = {
        let cancel = cancel_token.clone();
        let name = printer_name_owned.clone();
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor_printer_changes(&name, SLOW_POLL_INTERVAL_MS, Some(cancel), |changes| {
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
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor_property(
                    &name,
                    MonitorableProperty::IsOffline,
                    SLOW_POLL_INTERVAL_MS,
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
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor_property(
                    &name,
                    MonitorableProperty::Status,
                    SLOW_POLL_INTERVAL_MS,
                    Some(cancel),
                    |change| {
                        println!("   [Status Monitor] {}", change.description());
                    },
                )
                .await
        })
    };

    println!(
        "Three monitors started. Will cancel all in {} seconds...",
        COORDINATED_CANCEL_AFTER.as_secs()
    );
    sleep(COORDINATED_CANCEL_AFTER).await;

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
    monitor: &PrinterMonitor,
    printer_name: &str,
) -> Result<(), PrinterError> {
    println!(
        "Monitoring '{}' and will auto-cancel if printer goes offline or has an error...",
        printer_name
    );
    println!(
        "(Or after {} seconds, whichever comes first)",
        CONDITIONAL_TIMEOUT.as_secs()
    );

    let cancel_token = CancellationToken::new();
    let cancel_for_monitor = cancel_token.clone();
    let cancel_for_timeout = cancel_token.clone();

    // Start monitoring - both spawns share the outer monitor via Arc clone.
    let printer_name_owned = printer_name.to_string();
    let monitor_task = {
        let name = printer_name_owned.clone();
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor
                .monitor_printer_changes(
                    &name,
                    FAST_POLL_INTERVAL_MS,
                    Some(cancel_for_monitor),
                    |changes| {
                        let timestamp = changes.timestamp.format("%H:%M:%S");
                        // `monitor_printer_changes` only fires when properties
                        // actually changed - emit a single "changes detected"
                        // line instead of the misleading "active + maybe changes"
                        // pair that the previous form printed every poll.
                        println!("[{}] Changes: {}", timestamp, changes.summary());
                    },
                )
                .await
        })
    };

    // Check printer state periodically and cancel if offline or error. The
    // `tokio::select!` here makes the loop responsive to cancellation: if the
    // token fires (e.g. the monitor task hit its consecutive-error limit, or
    // the timeout task triggered) we exit within the same scheduler tick
    // instead of waiting up to `CONDITIONAL_CHECK_INTERVAL` for the next tick.
    let state_check_task = {
        let name = printer_name_owned.clone();
        let cancel = cancel_token.clone();
        let monitor = monitor.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(CONDITIONAL_CHECK_INTERVAL);

            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    _ = cancel.cancelled() => break,
                }

                if let Some(printer) = monitor.find_printer_cancellable(&name, None).await? {
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
            }

            Ok::<(), PrinterError>(())
        })
    };

    let timeout_task = tokio::spawn(async move {
        sleep(CONDITIONAL_TIMEOUT).await;
        if !cancel_for_timeout.is_cancelled() {
            println!(
                "\nTimeout reached ({} seconds). Cancelling monitoring...",
                CONDITIONAL_TIMEOUT.as_secs()
            );
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
