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
printer_event_handler = "1.3.0"
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

### Advanced Property Monitoring

The library supports detailed property-level monitoring to detect specific changes:

```rust
use printer_event_handler::PrinterMonitor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = PrinterMonitor::new().await?;
    
    // Monitor all property changes with detailed tracking
    monitor.monitor_printer_changes("HP LaserJet", 30, |changes| {
        if changes.has_changes() {
            println!("🔄 {} changes detected in '{}':", 
                changes.change_count(), changes.printer_name);
            
            for change in &changes.changes {
                println!("  - {}", change.description());
            }
        }
    }).await?;
    
    Ok(())
}
```

#### Monitor Specific Properties

```rust
// Monitor only offline status changes
monitor.monitor_property("HP LaserJet", "IsOffline", 60, |change| {
    println!("📶 Offline status: {}", change.description());
}).await?;

// Monitor multiple printers concurrently
let printer_names = vec!["HP LaserJet".to_string(), "Canon Printer".to_string()];
monitor.monitor_multiple_printers(printer_names, 30, |changes| {
    println!("🖨️  Multi-printer change: {} - {}", 
        changes.printer_name, changes.summary());
}).await?;
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

[2024-01-15 14:33:15] Printer 'HP LaserJet Pro' Status Changed:
  Status: Printing -> Busy
  Error State: No Error -> No Error
  Offline: No
```

## Platform Support

| Platform | Backend | Requirements | Coverage |
|----------|---------|--------------|----------|
| **Windows** | WMI (Windows Management Instrumentation) | None (built-in) | **Complete Win32_Printer support** - Full .NET PrintQueueStatus flag support (values like 1024, 16384) and 12 DetectedErrorState values (0-11) https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printer |
| **Linux** | CUPS (Common Unix Printing System) | `cups-client` package recommended | Basic status detection (idle, printing, offline) with CUPS integration |

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
- **`Printer`** - Represents a printer with complete WMI information and current state
- **`PrinterStatus`** - Printer status enum (current property, values 1-7)
- **`PrinterState`** - Printer state enum (.NET PrintQueueStatus flags like 1024, 16384)
- **`ErrorState`** - Error condition enum (NoError, Jammed, NoPaper, etc.)
- **`PrinterError`** - Error type for all operations

### Complete WMI Property Access

The `Printer` struct provides comprehensive access to all Win32_Printer WMI properties:

#### Raw Status Code Methods
```rust
// Get numeric WMI status codes
printer.printer_status_code()                    // Option<u32> - PrinterStatus (1-7)
printer.printer_state_code()                     // Option<u32> - PrinterState (.NET PrintQueueStatus flags)
printer.detected_error_state_code()              // Option<u32> - DetectedErrorState (0-11)
printer.extended_printer_status_code()           // Option<u32> - ExtendedPrinterStatus
printer.extended_detected_error_state_code()     // Option<u32> - ExtendedDetectedErrorState
printer.wmi_status()                             // Option<&str> - Status property
```

#### Human-Readable Description Methods
```rust
// Get human-readable descriptions for status codes
printer.printer_status_description()             // Option<&'static str>
printer.printer_state_description()              // Option<&'static str>
printer.detected_error_state_description()       // Option<&'static str>
printer.extended_printer_status_description()    // Option<&'static str>
```

#### WMI Status Values
The `wmi_status()` method returns the WMI Status property with values like:
- `"OK"` - Normal functioning
- `"Degraded"` - Functioning but with issues
- `"Error"` - Has problems
- `"Unknown"` - Cannot determine status
- `"No Contact"` - Communication lost
- And others per Microsoft documentation

#### Example: Detailed Printer Analysis
```rust
let printer = monitor.find_printer("HP Printer").await?.unwrap();

// Processed high-level information
println!("Name: {}", printer.name());
println!("Status: {}", printer.status_description());
println!("Offline: {}", printer.is_offline());

// Raw WMI analysis
println!("--- WMI Details ---");
if let Some(code) = printer.printer_status_code() {
    println!("PrinterStatus: {} ({})", code, 
        printer.printer_status_description().unwrap_or("Unknown"));
}

if let Some(code) = printer.extended_printer_status_code() {
    println!("ExtendedPrinterStatus: {} ({})", code,
        printer.extended_printer_status_description().unwrap_or("Unknown"));
}

if let Some(status) = printer.wmi_status() {
    println!("WMI Status: {}", status);
}
```

### Printer Status Values

The library provides comprehensive support for all Win32_Printer states:

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
Based on [.NET System.Printing.PrintQueueStatus](https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus):

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

**Note**: The PrinterState values are bitwise flags, so multiple states can be active simultaneously. The library automatically selects the most significant/priority state for display.

### Error States

Complete support for Win32_Printer DetectedErrorState values:

```rust
pub enum ErrorState {
    NoError,         // No issues (values 0, 2)
    Other,           // Other error (values 1, 9)
    LowPaper,        // Low paper (value 3)
    NoPaper,         // Out of paper (value 4)
    LowToner,        // Low toner/ink (value 5)
    NoToner,         // Out of toner/ink (value 6)
    DoorOpen,        // Cover/door open (value 7)
    Jammed,          // Paper jam (value 8)
    ServiceRequested,// Needs maintenance (value 10)
    OutputBinFull,   // Output tray full (value 11)
    UnknownError,    // Unknown error state
}
```

## Examples

Check out the [examples](examples/) directory for comprehensive usage patterns with complete WMI property access:

- [`basic_listing.rs`](examples/basic_listing.rs) - List all printers with detailed WMI information
- [`monitor_changes.rs`](examples/monitor_changes.rs) - Monitor status changes with WMI details
- [`property_monitoring.rs`](examples/property_monitoring.rs) - Advanced property-level change detection
- [`error_handling.rs`](examples/error_handling.rs) - Proper error handling and graceful degradation
- [`async_patterns.rs`](examples/async_patterns.rs) - Advanced async usage and concurrent monitoring

### Running Examples
Examples have their own Cargo.toml to keep the main library lightweight:

```bash
cd examples
cargo run --bin basic_listing
cargo run --bin monitor_changes -- "Printer Name"
```

Or from the main directory:
```bash
cargo run --manifest-path examples/Cargo.toml --bin basic_listing
```

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