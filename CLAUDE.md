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

The main functionality is in `src/main.rs` with a `monitor_windows_printers()` async function that:
1. Establishes WMI connection
2. Queries `Win32_Printer` WMI class
3. Returns printer information including status, error state, and offline status

The application is currently in early development with a placeholder main function.