# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Windows printer event monitoring application written in Rust. The project uses WMI (Windows Management Instrumentation) to query and monitor printer status and events on Windows systems.

## Development Commands

### Build
```bash
cargo build
```

### Run
```bash
cargo run
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

The application is structured as a simple async Rust application with the following key components:

- **WMI Integration**: Uses the `wmi` crate to interface with Windows Management Instrumentation for printer queries
- **Windows API**: Leverages Windows crate for direct Win32 printing APIs (`EnumPrintersW`, `GetPrinterW`)
- **Async Runtime**: Built on Tokio for asynchronous operations
- **Data Structures**: Defines `Win32_Printer` struct to represent printer information from WMI queries

### Key Dependencies

- `wmi`: Windows Management Instrumentation interface
- `windows`: Win32 API bindings for direct printer enumeration
- `tokio`: Async runtime with full features
- `serde_json`: JSON serialization support
- `log`/`env_logger`: Logging infrastructure
- `thiserror`/`anyhow`: Error handling

### Current Implementation

The application provides two main modes of operation:

#### List Mode (default)
When run without arguments, lists all printers on the system once:
```bash
cargo run
```

#### Monitor Mode  
When run with a printer name, monitors that specific printer every 60 seconds:
```bash
cargo run "Printer Name"
```

### Key Functions

- `check_printer_status()`: Queries all printers via WMI
- `find_printer_by_name()`: Finds a specific printer by name (case-insensitive)
- `monitor_printer_service()`: Continuously monitors a printer every 60 seconds
- Status change detection with timestamped logging
- Graceful error handling for missing printers

### Usage Examples

```bash
# List all printers
cargo run

# Monitor HP printer
cargo run "HPDC7777 (HP Smart Tank 580-590 series)"

# Monitor default PDF printer  
cargo run "Microsoft Print to PDF"
```