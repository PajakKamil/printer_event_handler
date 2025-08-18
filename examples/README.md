# Examples

This directory contains comprehensive examples demonstrating various usage patterns of the printer_event_handler library, including complete WMI property access.

## Running Examples

The examples have their own Cargo.toml to avoid adding unnecessary dependencies to the main library.

### From the examples directory:
```bash
cd examples

# List all printers with detailed WMI information
cargo run --bin basic_listing

# Monitor a specific printer for changes
cargo run --bin monitor_changes -- "Printer Name"
cargo run --bin monitor_changes  # Uses first available printer

# Demonstrate error handling patterns
cargo run --bin error_handling

# Advanced async patterns and concurrent monitoring
cargo run --bin async_patterns

# Property-level change monitoring
cargo run --bin property_monitoring -- "Printer Name"
cargo run --bin property_monitoring  # Uses first available printer
```

### From the main project directory:
```bash
# Run examples using cargo run with manifest path
cargo run --manifest-path examples/Cargo.toml --bin basic_listing
cargo run --manifest-path examples/Cargo.toml --bin monitor_changes -- "Printer Name"
cargo run --manifest-path examples/Cargo.toml --bin error_handling
cargo run --manifest-path examples/Cargo.toml --bin async_patterns
cargo run --manifest-path examples/Cargo.toml --bin property_monitoring -- "Printer Name"
```

## Example Files

### [`basic_listing.rs`](basic_listing.rs)
Demonstrates how to list all printers with complete WMI property access:
- Raw status codes (PrinterStatus, PrinterState, etc.)
- Human-readable descriptions
- WMI Status property values
- Summary statistics

**Features shown:**
- `printer_status_code()` and `printer_status_description()`
- `printer_state_code()` and `printer_state_description()`
- `detected_error_state_code()` and `detected_error_state_description()`
- `extended_printer_status_code()` and `extended_printer_status_description()`
- `wmi_status()` for system-level status assessment

### [`monitor_changes.rs`](monitor_changes.rs) 
Shows how to monitor printer status changes with detailed change detection:
- Real-time status monitoring
- Detailed WMI property change detection
- Status comparison between checks
- Command-line printer selection

**Features shown:**
- Status change detection with WMI details
- Before/after comparison of all WMI properties
- Timestamp logging
- Graceful handling of printer availability

### [`error_handling.rs`](error_handling.rs)
Comprehensive error handling patterns:
- Basic error handling with match statements
- Retry logic with exponential backoff
- Graceful degradation when features unavailable
- Platform-specific error handling (Windows WMI, Linux CUPS)

**Features shown:**
- WMI-specific error handling
- Graceful fallbacks when WMI properties unavailable
- Error recovery strategies
- Platform-specific debugging tips

### [`async_patterns.rs`](async_patterns.rs)
Advanced async usage patterns:
- Concurrent printer monitoring
- Streaming status updates with channels
- Background monitoring with shared state
- Multi-printer concurrent analysis

**Features shown:**
- Concurrent access to WMI properties
- Health scoring based on WMI status
- Background state management
- Stream processing of printer updates

### [`property_monitoring.rs`](property_monitoring.rs)
Property-level change monitoring and detection:
- Individual property change tracking
- Detailed change descriptions
- Specific property monitoring (e.g., only "IsOffline" changes)
- Multi-printer concurrent monitoring

**Features shown:**
- `monitor_printer_changes()` for detailed change detection
- `monitor_property()` for specific property watching
- `monitor_multiple_printers()` for concurrent monitoring
- `PropertyChange` and `PrinterChanges` types
- Property-specific callbacks and filtering

## Key WMI Properties Demonstrated

All examples showcase the complete set of WMI properties available:

### Raw Status Codes
- **PrinterStatus** (1-7) - Current/recommended property
- **PrinterState** (0-25) - Obsolete but detailed property  
- **DetectedErrorState** (0-11) - Error condition codes
- **ExtendedPrinterStatus** - Extended status information
- **ExtendedDetectedErrorState** - Extended error information

### Status Descriptions
- Human-readable descriptions for all numeric codes
- Fallback handling when descriptions unavailable
- Practical interpretation of status combinations

### WMI Status Property
- System-level status assessment ("OK", "Degraded", "Error", etc.)
- Integration with offline detection logic
- Health assessment patterns

## Usage Tips

1. **Start with `basic_listing.rs`** to understand available WMI properties
2. **Use `monitor_changes.rs`** to see real-time status updates
3. **Try `property_monitoring.rs`** for detailed property-level change tracking
4. **Study `error_handling.rs`** for production-ready error handling
5. **Explore `async_patterns.rs`** for advanced concurrent usage

## Platform Notes

- **Windows**: All WMI properties fully available
- **Linux**: Basic status detection with CUPS integration
- Examples gracefully handle platform differences