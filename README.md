# Printer Event Handler

A cross-platform Rust library for monitoring printer status and events on Windows and Linux systems.

[![Crates.io](https://img.shields.io/crates/v/printer_event_handler.svg)](https://crates.io/crates/printer_event_handler)
[![Documentation](https://docs.rs/printer_event_handler/badge.svg)](https://docs.rs/printer_event_handler)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

## Features

- **Cross-platform** - Windows (WMI) and Linux (CUPS) with a single async API.
- **Fluent builder** - `MonitorBuilder` collapses interval/cancellation/property-filter/event-mode options behind one chainable entry point.
- **Real-time monitoring** - millisecond polling, or sub-second event-driven monitoring via the optional `events` cargo feature (WMI `__InstanceModificationEvent` on Windows, `org.cups.cupsd.Notifier` D-Bus signals on Linux).
- **Cancellable** - every monitor and backend query accepts a `tokio_util::sync::CancellationToken`. `tokio::select!` cancellation arms are biased so an already-cancelled token always wins the race.
- **Stream API** - terminal methods return `tokio_stream::Stream<Item = Result<PrinterChanges>>` / `Stream<Item = Result<PropertyChange>>`. A terminal backend failure (e.g. sustained WMI/CUPS outage) propagates as the stream's final item before it closes, so callers can distinguish graceful shutdown from a crash.
- **Print job tracking** - `list_jobs` returns typed `Job` / `JobStatus` values from `Win32_PrintJob` on Windows, `lpstat -l -o` on Linux, or libcups2 FFI when the optional `linux-libcups` feature is on.
- **Rich Linux state** - CUPS `printer-state-reasons` parsed into the same `ErrorState` / `PrinterState` surface used on Windows (media-empty, toner-low, cover-open, jammed, etc.).
- **Typed task panics** - `monitor_multiple_printers` surfaces per-printer task panics as `PrinterError::TaskPanicked { printer_name, panic_message }`; no string parsing needed.
- **Optional `serde` / `tracing`** - off by default; opt in via cargo features.
- **Library + CLI** - use as a crate or as the `printer_monitor` binary.

## Quick Start

### As a Library

Add this to your `Cargo.toml`:

```toml
[dependencies]
printer_event_handler = "2.0.0"
tokio = { version = "1.0", features = ["full"] }
```

### Basic Listing

```rust
use printer_event_handler::{PrinterError, PrinterMonitor};

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    let monitor = PrinterMonitor::new().await?;

    // Cancellable variant - pass None when you don't need cancellation.
    let printers = monitor.list_printers_cancellable(None).await?;
    for printer in &printers {
        println!("{}: {}", printer.name(), printer.status_description());
        if printer.has_error() {
            println!("  Error: {}", printer.error_description());
        }
    }

    Ok(())
}
```

### Find a Specific Printer

```rust
use printer_event_handler::PrinterMonitor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;

    match monitor.find_printer_cancellable("Microsoft Print to PDF", None).await? {
        Some(printer) => {
            println!("Found: {}", printer.name());
            println!("Status:  {}", printer.status_description());
            println!("Default: {}", printer.is_default());
            println!("Offline: {}", printer.is_offline());
        }
        None => println!("Printer not found"),
    }

    Ok(())
}
```

### Monitor With the Fluent Builder

`PrinterMonitor::monitor(name)` returns a `MonitorBuilder` you can configure with chainable methods, then terminate with `run_changes` / `run_printer` / `run_property`:

```rust
use printer_event_handler::{MonitorableProperty, PrinterMonitor};
use tokio_util::sync::CancellationToken;

const INTERVAL_MS: u64 = 30_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    let cancel = CancellationToken::new();

    monitor
        .monitor("HP LaserJet Pro")
        .interval_ms(INTERVAL_MS)
        .cancel_token(cancel.clone())
        .run_changes(|changes| {
            if changes.has_changes() {
                println!(
                    "{} property change(s) on {}",
                    changes.change_count(),
                    changes.printer_name,
                );
                for change in &changes.changes {
                    println!("  - {}", change.description());
                }
            }
        })
        .await?;

    Ok(())
}
```

### Monitor a Single Property

```rust
use printer_event_handler::{MonitorableProperty, PrinterMonitor};

const INTERVAL_MS: u64 = 60_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;

    monitor
        .monitor("HP LaserJet")
        .interval_ms(INTERVAL_MS)
        .filter_property(MonitorableProperty::IsOffline)
        .run_property(|change| println!("Offline flipped: {}", change.description()))
        .await?;

    Ok(())
}
```

### Stream-Based Monitoring

`run_changes_stream` and `run_property_stream` return `tokio_stream::Stream` so you can pipe events through combinators instead of using a callback. Items are `Result<T>`: an `Err` is emitted once when the underlying monitor exits because of sustained backend failure, then the stream closes. A clean shutdown (cancellation, receiver drop) closes the stream without emitting an error.

```rust
use printer_event_handler::PrinterMonitor;
use tokio_stream::StreamExt;

const INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;

    let mut stream = monitor
        .monitor("HP LaserJet")
        .interval_ms(INTERVAL_MS)
        .run_changes_stream();

    while let Some(item) = stream.next().await {
        match item {
            Ok(changes) => println!("got {} change(s)", changes.change_count()),
            Err(e) => eprintln!("monitor stopped: {}", e),
        }
    }

    Ok(())
}
```

### Event-Driven Monitoring (opt-in)

With the `events` cargo feature enabled, the builder can subscribe to platform event notifications instead of polling: WMI `__InstanceModificationEvent` on Windows, `org.cups.cupsd.Notifier` D-Bus signals on Linux. State changes propagate within ~1 second of the platform notification. Without the feature, `with_events(true)` is accepted silently and the builder falls back to polling.

```toml
[dependencies]
printer_event_handler = { version = "2.0.0", features = ["events"] }
```

```rust
use printer_event_handler::PrinterMonitor;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;

    let mut stream = monitor
        .monitor("HP LaserJet")
        .with_events(true)
        .run_changes_stream();

    while let Some(item) = stream.next().await {
        match item {
            Ok(changes) => println!("event: {}", changes.summary()),
            Err(e) => eprintln!("subscription ended: {}", e),
        }
    }

    Ok(())
}
```

### Monitoring Multiple Printers

```rust
use printer_event_handler::PrinterMonitor;

const INTERVAL_MS: u64 = 30_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    let names = vec!["HP LaserJet".to_string(), "Canon Printer".to_string()];

    monitor
        .monitor_multiple_printers(names, INTERVAL_MS, None, |changes| {
            println!("{} - {}", changes.printer_name, changes.summary());
        })
        .await?;

    Ok(())
}
```

If a per-printer task panics, the call returns `PrinterError::TaskPanicked { printer_name, panic_message }` so you can match on the failing printer's name without parsing strings.

### Print Job Tracking

```rust
use printer_event_handler::PrinterMonitor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;

    // None = all printers; Some(name) = only that printer's jobs.
    let jobs = monitor.list_jobs(None, None).await?;
    for job in jobs {
        println!(
            "job {} on {:?}: {:?} (owner {:?})",
            job.job_id(),
            job.printer_name(),
            job.status(),
            job.owner(),
        );
    }

    Ok(())
}
```

On Linux the parser reads `lpstat -l -o` and maps the `Status:` line plus IPP `job-state-reasons` into `JobStatus` (`Printing`, `Spooling`, `Paused`, `Complete`, `Deleted`, `Error`, ...). With the optional `linux-libcups` cargo feature, the same call goes through libcups2 via `cupsGetJobs2()` instead of forking a subprocess - faster, and surfaces structured fields like job title and owner directly.

## Cargo Features

| Feature           | Default | Effect                                                                                                                                                |
|-------------------|---------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| `rt-multi-thread` | on      | Pulls in tokio's multi-threaded runtime. Disable for library-only consumers that bring their own runtime.                                             |
| `serde`           | off     | Adds `Serialize` / `Deserialize` derives to the public domain types (`Printer`, `Job`, status enums, change types).                                   |
| `tracing`         | off     | Routes library log calls through the `tracing` crate instead of `log`.                                                                                |
| `events`          | off     | Event-driven monitoring: WMI `__InstanceModificationEvent` on Windows, CUPS D-Bus signals (`org.cups.cupsd.Notifier`) on Linux. Enables `MonitorBuilder::with_events`. |
| `linux-libcups`   | off     | Linux only. Replaces the `lpstat` subprocess parser with a libcups2 FFI backend (`cupsGetDests2` / `cupsGetJobs2`). Requires `libcups2-dev` / `cups-devel` at build time. |

Example:

```toml
printer_event_handler = { version = "2.0.0", default-features = false, features = ["serde", "tracing"] }
tokio = { version = "1.0", features = ["macros", "rt"] }
```

## Cancellation

Every monitor and the cancellable backend methods accept `Option<CancellationToken>`. Cancellation is checked both before each poll and inside the sleep `tokio::select!`, so it stays responsive mid-interval.

```rust
use printer_event_handler::PrinterMonitor;
use tokio_util::sync::CancellationToken;

const INTERVAL_MS: u64 = 5_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    let cancel = CancellationToken::new();

    let cancel_for_signal = cancel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        cancel_for_signal.cancel();
    });

    monitor
        .monitor("HP LaserJet")
        .interval_ms(INTERVAL_MS)
        .cancel_token(cancel)
        .run_changes(|c| println!("{}", c.summary()))
        .await?;

    Ok(())
}
```

`list_printers_cancellable` / `find_printer_cancellable` return `PrinterError::Cancelled` when the token wins the race.

## Migrating From 1.x

2.0 is a breaking release. Highlights:

- **Removed**: `PrinterMonitor::list_printers` and `PrinterMonitor::find_printer`. Use `list_printers_cancellable(None)` and `find_printer_cancellable(name, None)` - pass `Some(token)` to abort the query.
- **Stream item type changed**: `run_changes_stream` / `run_property_stream` now yield `Result<T>` instead of `T`. A terminal `Err` propagates a sustained backend failure before the stream closes; clean shutdowns close the stream silently.
- **`PrinterBackend::list_jobs` is now required.** Downstream backend implementations can no longer silently fall back to an empty-vec default.
- **All public enums are `#[non_exhaustive]`** (`PrinterError`, `PrinterState`, `ErrorState`, `PrinterStatus`, `JobStatus`, `MonitorableProperty`, `PropertyChange`). Exhaustive `match`es need a wildcard arm; future variant additions are non-breaking within 2.x.
- **New typed error variants**: `PrinterError::Cancelled` (returned by the `*_cancellable` methods when the token wins the race) and `PrinterError::TaskPanicked { printer_name, panic_message }` (surfaced by `monitor_multiple_printers` so callers can match on the failing printer instead of parsing strings).

The positional `monitor_printer` / `monitor_printer_changes` / `monitor_property` methods still exist and are not deprecated; `MonitorBuilder` is a convenience layer on top of them.

## CLI Usage

The crate ships a `printer_monitor` binary:

```bash
# Install from crates.io
cargo install printer_event_handler

# Or run from source
git clone https://github.com/PajakKamil/printer_event_handler
cd printer_event_handler
```

### List All Printers

```bash
cargo run
```

Sample output:

```
Printer Status Checker
======================
Found 3 printer(s):

Printer #1: HP LaserJet Pro MFP M428f
  Status: Idle
  Error State: No Error
  Offline: No
  Default Printer: Yes

Printer #2: HPDC7777 (HP Smart Tank 580-590 series)
  Status: Offline
  Error State: Service Requested
  Offline: Yes

Printer #3: Microsoft Print to PDF
  Status: Idle
  Error State: No Error
  Offline: No
```

### Monitor a Specific Printer

```bash
cargo run -- "HP LaserJet Pro"
```

Sample output:

```
Printer Status Monitor Service
==============================
Monitoring printer 'HP LaserJet Pro' every 60 seconds...
Press Ctrl+C to stop

[2026-05-17 14:30:15] Printer 'HP LaserJet Pro' Initial Status:
  Status: Idle
  Error State: No Error
  Offline: No

[2026-05-17 14:31:15] Checking printer 'HP LaserJet Pro'
[2026-05-17 14:32:15] Printer 'HP LaserJet Pro' Status Changed:
  Status: Idle -> Printing
  Error State: No Error -> No Error
  Offline: No
```

## Platform Support

| Platform    | Backend                            | Requirements                       | Coverage                                                                                                                                                                                                                                              |
|-------------|------------------------------------|------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **Windows** | WMI (Win32_Printer + Win32_PrintJob) | None (built-in)                  | Full .NET PrintQueueStatus flag support and 12 DetectedErrorState values (0-11). Optional event-driven monitoring via `__InstanceModificationEvent` (cargo feature `events`). Print jobs via WMI. |
| **Linux**   | CUPS (`lpstat`)                    | `cups-client` package recommended  | Status from `lpstat -l -p`, including IPP `printer-state-reasons` mapped to typed `ErrorState` / `PrinterState`. Print jobs via `lpstat -l -o`. Subprocess calls run under `LANG=C` with a 5-second timeout. |

### Linux Setup

Ubuntu/Debian:

```bash
sudo apt install cups
# Add the libcups development headers if you plan to build with --features linux-libcups
sudo apt install libcups2-dev
```

RHEL/CentOS/Fedora:

```bash
sudo dnf install cups-client
# For --features linux-libcups
sudo dnf install cups-devel
```

## API Reference

### Core Types

- **`PrinterMonitor`** - main entry point. Cheaply `Clone`able (`Arc<dyn PrinterBackend>` inside) so multiple tasks can share one backend connection.
- **`MonitorBuilder`** - fluent configuration for per-printer monitoring runs.
- **`Printer`** - represents a printer plus all platform-specific raw codes.
- **`Job`** / **`JobStatus`** - typed print-job snapshot returned by `list_jobs`.
- **`MonitorableProperty`** - type-safe enum naming each monitorable property.
- **`PrinterStatus`** - operational status enum (values 1-7).
- **`PrinterState`** - .NET PrintQueueStatus flags (`PaperJam`, `TonerLow`, `DoorOpen`, ...).
- **`ErrorState`** - DetectedErrorState enum (`NoError`, `Jammed`, `NoPaper`, ...).
- **`PrinterChanges`** / **`PropertyChange`** - diff types emitted by change monitors.
- **`PrinterError`** - error enum (`WmiError`, `CupsError`, `PrinterNotFound`, `PlatformNotSupported`, `IoError`, `Cancelled`, `TaskPanicked { printer_name, panic_message }`, `Other`).
- **`CancellationToken`** - re-exported from `tokio_util::sync` for convenience.

### MonitorBuilder Methods

| Method                         | Effect                                                                                                                          |
|--------------------------------|---------------------------------------------------------------------------------------------------------------------------------|
| `interval_ms(ms)`              | Polling interval. Default: 60 000 ms.                                                                                           |
| `cancel_token(token)`          | Attach a `CancellationToken`.                                                                                                   |
| `wait_for_appearance(bool)`    | When `false`, return `PrinterError::PrinterNotFound` on the first poll if the printer is missing. Default `true` (wait silently). |
| `filter_property(prop)`        | Required by `run_property` / `run_property_stream`. Filters change events to a single property.                                 |
| `with_events(bool)`            | Use WMI event subscription when on Windows with `events` feature; falls back to polling otherwise.                              |
| `run_changes(callback)`        | Callback receives a `PrinterChanges` per poll that detected mutations.                                                          |
| `run_printer(callback)`        | Callback receives `(current, previous)` snapshots.                                                                              |
| `run_property(callback)`       | Callback receives a single `PropertyChange` matching `filter_property`.                                                         |
| `run_changes_stream()`         | Returns `Stream<Item = Result<PrinterChanges>>`. Terminal backend failure is emitted as the stream's final `Err` item.          |
| `run_property_stream()`        | Returns `Result<Stream<Item = Result<PropertyChange>>>`. Outer `Result` errors if `filter_property` was not set; inner mirrors the changes-stream contract. |

### Available Properties to Monitor

```rust
pub enum MonitorableProperty {
    Name,                            // Printer name changes
    Status,                          // PrinterStatus enum changes (recommended)
    State,                           // PrinterState enum changes
    ErrorState,                      // ErrorState enum changes
    IsOffline,                       // Online/offline status changes
    IsDefault,                       // Default printer designation changes
    PrinterStatusCode,               // Raw PrinterStatus code changes (1-7)
    PrinterStateCode,                // Raw PrinterState code changes
    DetectedErrorStateCode,          // Raw DetectedErrorState code changes (0-11)
    ExtendedDetectedErrorStateCode,  // Raw ExtendedDetectedErrorState code changes
    ExtendedPrinterStatusCode,       // Raw ExtendedPrinterStatus code changes
    WmiStatus,                       // WMI Status property changes
}
```

### Polling Intervals

All monitoring functions take an interval in **milliseconds**. Common values:

- `100` - 0.1 s, high frequency
- `500` - 0.5 s, responsive
- `1000` - 1 s, standard
- `5000` - 5 s, moderate
- `30000` - 30 s, conservative
- `60000` - 1 minute, low frequency

### Raw WMI Property Access (Windows)

`Printer` preserves the raw WMI codes alongside the typed enums:

```rust
printer.printer_status_code()                    // Option<u32> - PrinterStatus (1-7)
printer.printer_state_code()                     // Option<u32> - PrinterState (.NET PrintQueueStatus flags)
printer.detected_error_state_code()              // Option<u32> - DetectedErrorState (0-11)
printer.extended_printer_status_code()           // Option<u32> - ExtendedPrinterStatus
printer.extended_detected_error_state_code()     // Option<u32> - ExtendedDetectedErrorState
printer.wmi_status()                             // Option<&str> - Status property
```

Each one has a matching `*_description()` helper that returns a human-readable `&'static str`.

#### WMI Status Values

`wmi_status()` mirrors Microsoft's documented Status values:

- `"OK"` - normal functioning
- `"Degraded"` - functioning with issues
- `"Error"` - has problems
- `"Unknown"` - cannot determine status
- `"No Contact"` - communication lost

#### Example: Detailed Analysis

```rust
let printer = monitor
    .find_printer_cancellable("HP Printer", None)
    .await?
    .expect("printer present");

println!("Name:    {}", printer.name());
println!("Status:  {}", printer.status_description());
println!("Offline: {}", printer.is_offline());

println!("--- WMI Details ---");
if let Some(code) = printer.printer_status_code() {
    println!(
        "PrinterStatus: {} ({})",
        code,
        printer.printer_status_description().unwrap_or("Unknown"),
    );
}
if let Some(code) = printer.extended_printer_status_code() {
    println!(
        "ExtendedPrinterStatus: {} ({})",
        code,
        printer.extended_printer_status_description().unwrap_or("Unknown"),
    );
}
if let Some(status) = printer.wmi_status() {
    println!("WMI Status: {}", status);
}
```

### Status Enums

#### PrinterStatus (Current Property, Values 1-7)

```rust
pub enum PrinterStatus {
    Other,           // Other status (1)
    Unknown,         // Unknown status (2)
    Idle,            // Ready to print (3)
    Printing,        // Currently printing (4)
    Warmup,          // Starting up/warming up (5)
    StoppedPrinting, // Stopped mid-job (6)
    Offline,         // Not available (7)
    StatusUnknown,   // Could not determine
}
```

#### PrinterState (.NET PrintQueueStatus Flags)

Based on [.NET System.Printing.PrintQueueStatus](https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus). The Linux backend now feeds the same enum via IPP `printer-state-reasons`.

```rust
pub enum PrinterState {
    None,                     // No status (0)
    Paused,                   // Printer paused (1)
    Error,                    // General error state (2)
    PendingDeletion,          // Queued for deletion (4)
    PaperJam,                 // Paper jam detected (8)
    PaperOut,                 // Out of paper (16)
    ManualFeed,               // Manual feed required (32)
    PaperProblem,             // Paper-related issue (64)
    Offline,                  // Not available (128)
    IOActive,                 // I/O operations active (256)
    Busy,                     // Printer busy (512)
    Printing,                 // Currently printing (1024)
    OutputBinFull,            // Output tray full (2048)
    NotAvailable,             // Printer not available (4096)
    Waiting,                  // Waiting for job (8192)
    Processing,               // Processing job (16384)
    Initializing,             // Initializing (32768)
    WarmingUp,                // Warming up (65536)
    TonerLow,                 // Low toner/ink (131072)
    NoToner,                  // Out of toner/ink (262144)
    PagePunt,                 // Page punt condition (524288)
    UserInterventionRequired, // User action needed (1048576)
    OutOfMemory,              // Memory full (2097152)
    DoorOpen,                 // Cover/door open (4194304)
    ServerUnknown,            // Server status unknown (8388608)
    PowerSave,                // Power save mode (16777216)
    StatusUnknown,            // Could not determine
}
```

**Note**: PrinterState values are bitwise flags, so multiple states can be active simultaneously. The library picks the most informative single variant via a priority chain (specific causes such as `PaperJam` or `DoorOpen` win over the generic `Error` bit).

#### ErrorState (Win32_Printer DetectedErrorState Values)

```rust
pub enum ErrorState {
    NoError,          // No issues (values 0, 2)
    Other,            // Other error (values 1, 9)
    LowPaper,         // Low paper (value 3)
    NoPaper,          // Out of paper (value 4)
    LowToner,         // Low toner/ink (value 5)
    NoToner,          // Out of toner/ink (value 6)
    DoorOpen,         // Cover/door open (value 7)
    Jammed,           // Paper jam (value 8)
    ServiceRequested, // Needs maintenance (value 10)
    OutputBinFull,    // Output tray full (value 11)
    UnknownError,     // Unknown error state
}
```

## Examples

The [examples](examples/) directory holds runnable usage patterns. Examples have their own `Cargo.toml` to keep the main library lightweight. See [examples/README.md](examples/README.md) for a recommended reading order.

- [`basic_listing.rs`](examples/basic_listing.rs) - list all printers with detailed information.
- [`monitor_changes.rs`](examples/monitor_changes.rs) - monitor status changes over time.
- [`property_monitoring.rs`](examples/property_monitoring.rs) - property-level change detection.
- [`streaming_changes.rs`](examples/streaming_changes.rs) - `run_changes_stream` / `run_property_stream` with `StreamExt` combinators.
- [`events_demo.rs`](examples/events_demo.rs) - `with_events(true)` for WMI / D-Bus event subscriptions.
- [`jobs_listing.rs`](examples/jobs_listing.rs) - `list_jobs` across all printers or a single queue.
- [`error_handling.rs`](examples/error_handling.rs) - graceful error handling, including `Cancelled` / `TaskPanicked` matching.
- [`async_patterns.rs`](examples/async_patterns.rs) - concurrent monitoring patterns.
- [`cancellation_token_example.rs`](examples/cancellation_token_example.rs) - graceful shutdown via `CancellationToken`.

Run from the repo root:

```bash
cargo run --manifest-path examples/Cargo.toml --bin basic_listing
cargo run --manifest-path examples/Cargo.toml --bin monitor_changes -- "Printer Name"
```

## Contributing

Contributions are welcome. For major changes, please open an issue first to discuss what you would like to change.

### Development

```bash
git clone https://github.com/PajakKamil/printer_event_handler
cd printer_event_handler

cargo test
cargo fmt
cargo clippy -- -D warnings
cargo doc --open

# Feature combinations
cargo test --features serde
cargo build --features tracing
cargo build --features events     # WMI events on Windows, CUPS D-Bus on Linux
cargo build --features linux-libcups   # Linux only; needs libcups2-dev / cups-devel
cargo build --no-default-features --lib
```

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for details about changes in each version.
