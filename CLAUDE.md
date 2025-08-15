# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a cross-platform printer event monitoring library and CLI application written in Rust. The project is structured as a reusable library crate that provides printer monitoring functionality through platform-specific backends:

- **Windows**: WMI (Windows Management Instrumentation)
- **Linux**: CUPS (Common Unix Printing System) via system commands

The library provides a unified API that works across both platforms, along with a command-line interface for direct usage.

## Development Commands

### Build
```bash
cargo build
```

### Run CLI
```bash
# List all printers
cargo run

# Monitor specific printer
cargo run -- "Printer Name"
```

### Run with binary name
```bash
# List all printers
cargo run --bin printer_monitor

# Monitor specific printer  
cargo run --bin printer_monitor -- "Printer Name"
```

### Development build (debug)
```bash
cargo build --debug
```

### Release build
```bash
cargo build --release
```

### Check code
```bash
cargo check
```

### Run tests
```bash
cargo test
```

### Format code
```bash
cargo fmt
```

### Lint code
```bash
cargo clippy
```

## Architecture

The project is structured as a library crate with both library and binary targets:

### Library Structure
- **src/lib.rs**: Main library entry point with public API
- **src/error.rs**: Custom error types (`PrinterError`) with proper error handling
- **src/printer.rs**: Core printer data structures (`Printer`, `PrinterStatus`, `ErrorState`)
- **src/monitor.rs**: Printer monitoring functionality (`PrinterMonitor`)
- **src/main.rs**: CLI application that uses the library

### Key Components
- **Cross-Platform Backend**: Unified trait-based abstraction for different platforms
- **Windows Integration**: Uses the `wmi` crate to interface with Windows Management Instrumentation
- **Linux Integration**: Uses CUPS system commands (`lpstat`) for printer detection and status monitoring
- **Async Runtime**: Built on Tokio for asynchronous operations and monitoring
- **Type Safety**: Strong typing with custom enums for printer status and error states
- **Error Handling**: Comprehensive error handling with custom `PrinterError` type
- **Platform Detection**: Automatic backend selection based on target platform

### Key Dependencies

- `wmi`: Windows Management Instrumentation interface (Windows only)
- `tokio`: Async runtime with full features (including process and fs for Linux)
- `async-trait`: Async trait support for cross-platform backend abstraction
- `serde`: Serialization support with derive features
- `log`/`env_logger`: Logging infrastructure
- `chrono`: Date/time handling for timestamps

### Platform-Specific Requirements

**Windows:**
- No additional system requirements - uses built-in WMI

**Linux:**
- Recommended: CUPS installed (`sudo apt install cups-client` on Ubuntu/Debian)
- Alternative: Basic printer detection via `/dev/lp0` and system files
- Commands used: `lpstat`, `which` (typically pre-installed)

## Library Usage

### As a Library Dependency

Add to your `Cargo.toml`:
```toml
[dependencies]
printer_event_handler = { path = "../printer_event_handler" }
# or from crates.io when published:
# printer_event_handler = "0.1.0"
```

### Library API

```rust
use printer_event_handler::{PrinterMonitor, PrinterError, Printer};

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    // Create a monitor instance
    let monitor = PrinterMonitor::new().await?;
    
    // List all printers
    let printers = monitor.list_printers().await?;
    for printer in printers {
        println!("Printer: {}", printer.name());
        println!("Status: {}", printer.status_description());
        println!("Has Error: {}", printer.has_error());
    }
    
    // Find specific printer
    if let Some(printer) = monitor.find_printer("My Printer").await? {
        println!("Found printer: {}", printer.name());
    }
    
    // Monitor printer with callback
    monitor.monitor_printer("My Printer", 60, |current, previous| {
        if let Some(prev) = previous {
            if prev != current {
                println!("Printer status changed!");
            }
        }
    }).await?;
    
    Ok(())
}
```

### Key Library Types

- **`PrinterMonitor`**: Main entry point for printer operations
- **`Printer`**: Represents a printer and its current state
- **`PrinterStatus`**: Enum for printer status (Idle, Printing, Offline, etc.)
- **`ErrorState`**: Enum for printer error conditions (NoError, Jammed, NoPaper, etc.)
- **`PrinterError`**: Custom error type for all library operations

## CLI Usage

The CLI provides two main modes of operation:

#### List Mode (default)
When run without arguments, lists all printers on the system once:
```bash
cargo run
```

#### Monitor Mode  
When run with a printer name, monitors that specific printer every 60 seconds:
```bash
cargo run -- "Printer Name"
```

### CLI Examples

```bash
# List all printers
cargo run

# Monitor HP printer
cargo run -- "HPDC7777 (HP Smart Tank 580-590 series)"

# Monitor default PDF printer  
cargo run -- "Microsoft Print to PDF"

# Using binary name explicitly
cargo run --bin printer_monitor -- "My Printer"
```

## Testing and Development

### Running Tests
```bash
# Run all tests (unit tests, integration tests, doc tests)
cargo test

# Run only library tests
cargo test --lib

# Run tests with output
cargo test -- --nocapture
```

### Building for Release
```bash
# Build optimized release version
cargo build --release

# The binary will be available at:
# target/release/printer_monitor.exe (Windows)
```

### Publishing as a Crate

When ready to publish to crates.io:
1. Update version in `Cargo.toml`
2. Add proper repository URL and description
3. Run `cargo publish --dry-run` to test
4. Run `cargo publish` to publish

## Platform Support

This library supports **Windows and Linux** with platform-specific backends:

- **Windows**: Uses WMI (Windows Management Instrumentation) for comprehensive printer information
- **Linux**: Uses CUPS system commands (`lpstat`) and alternative detection methods
- **Other platforms**: Will return `PrinterError::PlatformNotSupported`

### Backend Selection
The appropriate backend is automatically selected at compile time based on the target platform using Rust's conditional compilation features (`#[cfg(windows)]` and `#[cfg(unix)]`).