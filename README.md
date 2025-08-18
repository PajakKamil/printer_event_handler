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
printer_event_handler = "1.2.0"
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
| **Windows** | WMI (Windows Management Instrumentation) | None (built-in) | **Complete Win32_Printer support** - All 26 PrinterState values (0-25) and 12 DetectedErrorState values (0-11) https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printer |
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
- **`PrinterState`** - Printer state enum (obsolete property, values 0-25)
- **`ErrorState`** - Error condition enum (NoError, Jammed, NoPaper, etc.)
- **`PrinterError`** - Error type for all operations

### Complete WMI Property Access

The `Printer` struct provides comprehensive access to all Win32_Printer WMI properties:

#### Raw Status Code Methods
```rust
// Get numeric WMI status codes
printer.printer_status_code()                    // Option<u32> - PrinterStatus (1-7)
printer.printer_state_code()                     // Option<u32> - PrinterState (0-25, obsolete)
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

```rust
pub enum PrinterStatus {
    // Basic PrinterStatus values (1-7)
    Other,          // Other status
    Unknown,        // Unknown status  
    Idle,           // Ready to print
    Printing,       // Currently printing
    Warmup,         // Starting up/warming up
    StoppedPrinting,// Stopped mid-job
    Offline,        // Not available
    
    // Extended PrinterState values (0-25) - Full Win32_Printer support
    Paused,         // Printer paused
    Error,          // General error state
    PendingDeletion,// Queued for deletion
    PaperJam,       // Paper jam detected
    PaperOut,       // Out of paper
    ManualFeed,     // Manual feed required
    PaperProblem,   // Paper-related issue
    IOActive,       // I/O operations active
    Busy,           // Printer busy
    OutputBinFull,  // Output tray full
    NotAvailable,   // Printer not available
    Waiting,        // Waiting for job
    Processing,     // Processing job
    Initialization, // Initializing
    TonerLow,       // Low toner/ink
    NoToner,        // Out of toner/ink
    PagePunt,       // Page punt condition
    UserInterventionRequired, // User action needed
    OutOfMemory,    // Memory full
    DoorOpen,       // Cover/door open
    ServerUnknown,  // Server status unknown
    PowerSave,      // Power save mode
    
    StatusUnknown,  // Could not determine
}
```

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