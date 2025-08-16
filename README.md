# Printer Event Handler

A cross-platform Rust library for monitoring printer status and events on Windows and Linux systems.

[![Crates.io](https://img.shields.io/crates/v/printer_event_handler.svg)](https://crates.io/crates/printer_event_handler)
[![Documentation](https://docs.rs/printer_event_handler/badge.svg)](https://docs.rs/printer_event_handler)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

## Features

- 🖨️ **Cross-platform support** - Works on Windows (WMI) and Linux (CUPS)
- 📊 **Real-time monitoring** - Monitor printer status changes with customizable intervals
- 🔍 **Printer discovery** - List and find printers on your system
- ⚡ **Async/await support** - Built on Tokio for efficient asynchronous operations
- 🛡️ **Type-safe** - Strongly typed printer status and error states
- 📦 **Library + CLI** - Use as a library in your projects or as a standalone CLI tool

## Quick Start

### As a Library

Add this to your `Cargo.toml`:

```toml
[dependencies]
printer_event_handler = "1.0.0"
tokio = { version = "1.0", features = ["full"] }
```

### Basic Usage

```rust
use printer_event_handler::{PrinterMonitor, PrinterError};

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    // Create a printer monitor
    let monitor = PrinterMonitor::new().await?;
    
    // List all printers
    let printers = monitor.list_printers().await?;
    for printer in &printers {
        println!("📄 {}: {}", printer.name(), printer.status_description());
        if printer.has_error() {
            println!("   ⚠️  Error: {}", printer.error_description());
        }
    }
    
    Ok(())
}
```

### Monitor Printer Status Changes

```rust
use printer_event_handler::PrinterMonitor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    
    // Monitor a specific printer every 30 seconds
    monitor.monitor_printer("HP LaserJet Pro", 30, |current, previous| {
        if let Some(prev) = previous {
            if prev != current {
                println!("🔄 Status changed: {} → {}", 
                    prev.status_description(), 
                    current.status_description());
                
                if current.has_error() {
                    println!("❌ Error detected: {}", current.error_description());
                }
            }
        } else {
            println!("📊 Initial status: {}", current.status_description());
        }
    }).await?;
    
    Ok(())
}
```

### Find Specific Printer

```rust
use printer_event_handler::PrinterMonitor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    
    match monitor.find_printer("Microsoft Print to PDF").await? {
        Some(printer) => {
            println!("Found printer: {}", printer.name());
            println!("Status: {}", printer.status_description());
            println!("Is default: {}", printer.is_default());
            println!("Is offline: {}", printer.is_offline());
        }
        None => println!("Printer not found"),
    }
    
    Ok(())
}
```

### Check for Changes

```rust
use printer_event_handler::PrinterMonitor;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    
    // Get initial status
    let initial = monitor.find_printer("Your Printer").await?;
    
    // Wait and check again
    sleep(Duration::from_secs(5)).await;
    let current = monitor.find_printer("Your Printer").await?;
    
    // Compare states
    match (initial, current) {
        (Some(before), Some(after)) if before != after => {
            println!("Change detected!");
            println!("Before: {}", before.status_description());
            println!("After: {}", after.status_description());
        }
        (Some(_), Some(_)) => println!("No changes detected"),
        _ => println!("Printer not found"),
    }
    
    Ok(())
}
```

## CLI Usage

The crate also provides a command-line interface:

```bash
# Install from crates.io
cargo install printer_event_handler

# Or run directly from source
git clone https://github.com/PajakKamil/printer_event_handler
cd printer_event_handler
```

### List All Printers

```bash
cargo run
```

Output:
```
Printer Status Checker
======================
Found 3 printer(s):

Printer #1: HP LaserJet Pro MFP M428f
  Status: Idle
  Error State: No Error
  Offline: No
  Default Printer: Yes

Printer #2: Microsoft Print to PDF
  Status: Idle
  Error State: No Error
  Offline: No

Printer #3: Microsoft XPS Document Writer
  Status: Idle
  Error State: No Error
  Offline: No
```

### Monitor a Specific Printer

```bash
cargo run -- "HP LaserJet Pro"
```

Output:
```
Printer Status Monitor Service
==============================
Monitoring printer 'HP LaserJet Pro' every 60 seconds...
Press Ctrl+C to stop

[2024-01-15 14:30:15] Printer 'HP LaserJet Pro' Initial Status:
  Status: Idle
  Error State: No Error
  Offline: No

[2024-01-15 14:31:15] Checking printer 'HP LaserJet Pro'
[2024-01-15 14:32:15] Printer 'HP LaserJet Pro' Status Changed:
  Status: Idle -> Printing
  Error State: No Error -> No Error
  Offline: No
```

## Platform Support

| Platform | Backend | Requirements |
|----------|---------|--------------|
| **Windows** | WMI (Windows Management Instrumentation) | None (built-in) |
| **Linux** | CUPS (Common Unix Printing System) | `cups-client` package recommended |

### Linux Setup

On Ubuntu/Debian:
```bash
sudo apt install cups
```

On RHEL/CentOS/Fedora:
```bash
sudo yum install cups  # or dnf install cups-client
```

## API Reference

### Core Types

- **`PrinterMonitor`** - Main entry point for all printer operations
- **`Printer`** - Represents a printer and its current state
- **`PrinterStatus`** - Printer status enum (Idle, Printing, Offline, etc.)
- **`ErrorState`** - Error condition enum (NoError, Jammed, NoPaper, etc.)
- **`PrinterError`** - Error type for all operations

### Printer Status Values

```rust
pub enum PrinterStatus {
    Idle,           // Ready to print
    Printing,       // Currently printing
    Offline,        // Not available
    Warmup,         // Starting up
    StoppedPrinting,// Stopped mid-job
    Other,          // Other status
    Unknown,        // Unknown status
    StatusUnknown,  // Could not determine
}
```

### Error States

```rust
pub enum ErrorState {
    NoError,         // No issues
    Jammed,          // Paper jam
    NoPaper,         // Out of paper
    NoToner,         // Out of toner/ink
    DoorOpen,        // Cover/door open
    OutputBinFull,   // Output tray full
    ServiceRequested,// Needs maintenance
    LowPaper,        // Low paper
    LowToner,        // Low toner/ink
    Other,           // Other error
    UnknownError,    // Unknown error
}
```

## Examples

Check out the [examples](examples/) directory for more usage patterns:

- [`basic_listing.rs`](examples/basic_listing.rs) - List all printers
- [`monitor_changes.rs`](examples/monitor_changes.rs) - Monitor status changes
- [`error_handling.rs`](examples/error_handling.rs) - Proper error handling
- [`async_patterns.rs`](examples/async_patterns.rs) - Advanced async usage

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development

```bash
# Clone the repository
git clone https://github.com/PajakKamil/printer_event_handler
cd printer_event_handler

# Run tests
cargo test

# Check formatting
cargo fmt

# Run linter
cargo clippy

# Build documentation
cargo doc --open
```

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for details about changes in each version.