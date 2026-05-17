//! Event-Driven Monitoring Example
//!
//! Showcases `MonitorBuilder::with_events(true)` - 2.0's opt-in toggle for
//! the event-driven path:
//!
//! - **Windows**: WMI `__InstanceModificationEvent` subscription via
//!   `wmi::WMIConnection::async_raw_notification`.
//! - **Linux/macOS**: CUPS `org.cups.cupsd.Notifier` D-Bus signals via the
//!   `zbus` crate.
//!
//! Both paths require the parent crate's `events` cargo feature (already
//! enabled in `examples/Cargo.toml`). Without it the builder transparently
//! falls back to polling with a `warn!` log line - the example still works,
//! just at the polling cadence.
//!
//! `with_events(true)` doesn't change the callback or stream surface; you
//! still terminate with `run_changes`, `run_changes_stream`, etc. Events
//! arrive as the OS reports them (sub-second on Windows; whenever cupsd
//! emits a signal on Linux), so this path beats polling for latency-sensitive
//! observability while preserving the same `PrinterChanges` payload shape.
//!
//! Run with:
//! ```bash
//! cargo run --manifest-path examples/Cargo.toml --bin events_demo
//! cargo run --manifest-path examples/Cargo.toml --bin events_demo -- "Printer Name"
//! ```

use std::env;
use std::time::Duration;

use printer_event_handler::{CancellationToken, PrinterError, PrinterMonitor};
use tokio_stream::StreamExt;

/// Polling cadence used as a safety-net while the event subscription is
/// active. The event path doesn't actually poll, but the builder still
/// accepts `interval_ms` because the polling fallback (Linux without
/// `events`, Windows pre-event-feature, broker unreachable) honours it.
const SAFETY_INTERVAL_MS: u64 = 5_000;

/// Total wall-clock budget for the example. Long enough that the user can
/// flip the printer offline / back on and see events arrive; short enough
/// that the binary doesn't hang forever in CI.
const DEMO_DURATION: Duration = Duration::from_secs(20);

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Event-Driven Monitoring");
    println!("==================================================\n");

    print_platform_notice();

    let monitor = PrinterMonitor::new().await?;
    let target = resolve_target(&monitor).await?;
    println!("Subscribing for: {}", target);
    println!(
        "Will run for {:?}. Try toggling the printer offline / online to see events arrive.\n",
        DEMO_DURATION
    );

    let cancel = CancellationToken::new();
    let cancel_deadline = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(DEMO_DURATION).await;
        cancel_deadline.cancel();
    });

    // The `events` cargo feature flips the body of this call to the WMI /
    // D-Bus subscription path. Without the feature this becomes a polling
    // monitor and a one-shot warning is logged - the example still works,
    // just on the polling cadence.
    let mut stream = monitor
        .monitor(&target)
        .interval_ms(SAFETY_INTERVAL_MS)
        .with_events(true)
        .cancel_token(cancel)
        .run_changes_stream();

    let mut received = 0;
    while let Some(changes) = stream.next().await {
        received += 1;
        println!(
            "[{}] event #{}: {} change(s) on {}",
            changes.timestamp.format("%H:%M:%S"),
            received,
            changes.change_count(),
            changes.printer_name
        );
        for change in &changes.changes {
            println!("    - {}", change.description());
        }
    }

    println!("\nStream closed - {} event(s) observed.", received);
    Ok(())
}

/// One-shot heads-up describing which backend the binary will use. Helps
/// readers understand why latency / behaviour differs across platforms.
fn print_platform_notice() {
    #[cfg(all(windows, feature = "events"))]
    println!("Platform: Windows with `events` feature - using WMI subscription.\n");
    #[cfg(all(unix, feature = "events"))]
    println!("Platform: unix with `events` feature - using CUPS D-Bus subscription.\n");
    #[cfg(not(feature = "events"))]
    println!(
        "Note: built without the `events` cargo feature - falling back to polling at {} ms.\n",
        SAFETY_INTERVAL_MS
    );
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
