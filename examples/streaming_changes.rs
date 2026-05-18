//! Streaming Changes Example
//!
//! Showcases the 2.0 stream-flavoured terminal methods on
//! [`MonitorBuilder`]: `run_changes_stream` and `run_property_stream`. The
//! callback-driven `run_changes` / `run_property` paths still exist (see
//! `monitor_changes.rs`, `property_monitoring.rs`), but for code that wants
//! `StreamExt` adapters (`take`, `filter`, `timeout`, `merge`, ...) the
//! stream variants are the recommended form.
//!
//! Internally each stream is fed by a background task that drives the
//! existing monitor loop and forwards events through an
//! `mpsc::unbounded_channel`. The stream terminates when the monitor exits -
//! cancellation token fires, sustained backend failure, or the receiver is
//! dropped.
//!
//! Run with:
//! ```bash
//! cargo run --manifest-path examples/Cargo.toml --bin streaming_changes
//! cargo run --manifest-path examples/Cargo.toml --bin streaming_changes -- "Printer Name"
//! ```

use std::env;
use std::time::Duration;

use printer_event_handler::{CancellationToken, MonitorableProperty, PrinterError, PrinterMonitor};
use tokio_stream::StreamExt;

/// Poll cadence for the streamed monitor. Same value would feed the
/// callback-style API; switching forms doesn't change the underlying loop.
const STREAM_INTERVAL_MS: u64 = 1_000;

/// How long the example runs before cancelling itself. Cancellation closes
/// the underlying monitor, which in turn ends the stream - the `while let`
/// loop exits cleanly without any explicit `break`.
const STREAM_DURATION: Duration = Duration::from_secs(15);

/// Max change events to print before terminating Section 1, regardless of how
/// many fire in `STREAM_DURATION`. Caps log volume on a busy printer.
const MAX_CHANGE_EVENTS: usize = 20;

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Streaming Changes Example");
    println!("====================================================\n");

    let monitor = PrinterMonitor::new().await?;
    let target = resolve_target(&monitor).await?;
    println!("Streaming for: {}\n", target);

    println!("Section 1: run_changes_stream (full PrinterChanges per poll)");
    println!("---------------------------------------------------------");
    stream_changes(&monitor, &target).await?;

    println!("\nSection 2: run_property_stream (filtered to IsOffline)");
    println!("------------------------------------------------------");
    stream_property(&monitor, &target).await?;

    println!("\nStreaming example complete.");
    Ok(())
}

async fn resolve_target(monitor: &PrinterMonitor) -> Result<String, PrinterError> {
    if let Some(name) = env::args().nth(1) {
        if monitor
            .find_printer_cancellable(&name, None)
            .await?
            .is_some()
        {
            return Ok(name);
        }
        println!(
            "Warning: printer '{}' not found - falling back to first available",
            name
        );
    }

    let printers = monitor.list_printers_cancellable(None).await?;
    printers
        .first()
        .map(|p| p.name().to_string())
        .ok_or_else(|| PrinterError::Other("No printers found on this system".into()))
}

/// Demonstrates `run_changes_stream`. Each poll-detected mutation arrives as
/// a `PrinterChanges` value. We use `StreamExt::take` to cap volume and a
/// `CancellationToken` as the time budget.
async fn stream_changes(monitor: &PrinterMonitor, target: &str) -> Result<(), PrinterError> {
    let cancel = CancellationToken::new();
    spawn_deadline(cancel.clone(), STREAM_DURATION);

    let stream = monitor
        .monitor(target)
        .interval_ms(STREAM_INTERVAL_MS)
        .cancel_token(cancel.clone())
        .run_changes_stream();

    let mut stream = stream.take(MAX_CHANGE_EVENTS);
    let mut received = 0;
    // Stream items are `Result<PrinterChanges>` so callers can distinguish
    // graceful end (cancellation, receiver drop) from a terminal backend
    // failure - sustained WMI/CUPS errors propagate here as the final item.
    while let Some(item) = stream.next().await {
        match item {
            Ok(changes) => {
                received += 1;
                println!(
                    "  [{}] {} change(s): {}",
                    changes.timestamp.format("%H:%M:%S"),
                    changes.change_count(),
                    changes.summary()
                );
                for change in &changes.changes {
                    println!("      - {}", change.description());
                }
            }
            Err(e) => {
                println!("  Section 1 backend error (stream ending): {}", e);
                break;
            }
        }
    }
    println!(
        "  Section 1 ended ({} event(s) printed, cap = {}).",
        received, MAX_CHANGE_EVENTS
    );
    Ok(())
}

/// Demonstrates `run_property_stream`. The builder requires
/// `filter_property(...)` to be set before this terminal method - otherwise
/// it returns `PrinterError::Other` (the example exercises the happy path).
async fn stream_property(monitor: &PrinterMonitor, target: &str) -> Result<(), PrinterError> {
    let cancel = CancellationToken::new();
    spawn_deadline(cancel.clone(), STREAM_DURATION);

    let stream = monitor
        .monitor(target)
        .interval_ms(STREAM_INTERVAL_MS)
        .filter_property(MonitorableProperty::IsOffline)
        .cancel_token(cancel.clone())
        .run_property_stream()?;

    tokio::pin!(stream);
    while let Some(item) = stream.next().await {
        match item {
            Ok(change) => println!("  IsOffline change: {}", change.description()),
            Err(e) => {
                println!("  Section 2 backend error (stream ending): {}", e);
                break;
            }
        }
    }
    println!("  Section 2 ended (stream closed - cancellation or backend exit).");
    Ok(())
}

/// Cancel the given token after `deadline` elapses. Used so each section
/// terminates on its own without external Ctrl+C.
fn spawn_deadline(cancel: CancellationToken, deadline: Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(deadline).await;
        cancel.cancel();
    });
}
