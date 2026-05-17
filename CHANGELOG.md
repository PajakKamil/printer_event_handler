# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-05-17

Major release focused on Linux feature parity with the Windows backend.
Adds CUPS D-Bus event-driven monitoring, an opt-in libcups FFI backend,
~25 additional IPP `printer-state-reasons` mappings, and a unified Job
constructor that surfaces document titles on the libcups path. The
breaking changes the major bump unlocks are bundled here: `#[non_exhaustive]`
on every public enum, mandatory `PrinterBackend::list_jobs`, and removal
of the 1.5.0-deprecated `list_printers` / `find_printer` methods.

### Added

- **CUPS event-driven monitoring** - `MonitorBuilder::with_events(true)`
  now works on Linux/macOS, backed by `org.cups.cupsd.Notifier` D-Bus
  signals via the `zbus` crate. Failure modes (no D-Bus, cupsd built
  without `--enable-dbus`) silently fall back to polling, matching the
  Windows path's tolerance. Same `events` cargo feature gates both
  platforms.
- **`linux-libcups` cargo feature** - opt-in libcups2 FFI backend
  selected automatically by `create_backend()` when enabled. Faster
  than the default `lpstat` subprocess path, surfaces document titles
  on jobs, and unlocks future CUPS subscription work. Requires
  `libcups2-dev` / `cups-devel` at build time. Default Linux build keeps
  using `lpstat` so no system library is required.
- **Extended `printer-state-reasons` mapping** - 25+ additional IPP
  reason stems now decoded into typed `ErrorState` / `PrinterState`
  values (`media-path-jam`, `service-required`, `marker-supply-missing`,
  `fuser-over-temp`, `power-save`, `in-use`, `processing-to-stop-point`,
  etc.). Several `PrinterState` flags (`WarmingUp`, `Busy`, `Processing`,
  `Initializing`, `UserInterventionRequired`, `PowerSave`) now light up
  on Linux for the first time.
- **`Job::document` populated on libcups path** - `cups_job_t.title`
  feeds the `document` field, matching the Windows `Win32_PrintJob.Document`
  behaviour. Page counters still require IPP `Get-Job-Attributes` (planned
  follow-up).

### Changed (breaking)

- **`PrinterError`, `PrinterState`, `ErrorState`, `PrinterStatus`,
  `JobStatus` are now `#[non_exhaustive]`** - downstream exhaustive
  `match` arms need a wildcard arm to keep compiling. Future additive
  variants in 2.x are non-breaking.
- **`PrinterBackend::list_jobs` is now required** - the 1.5.0 default
  `Ok(vec![])` impl is gone; every backend must implement it explicitly.
- **`Job::from_lpstat` -> `Job::from_cups`** - unified constructor shared
  by the `lpstat` and `libcups` paths, with new optional fields
  (`document`, `total_pages`, `pages_printed`, `name`). Only affects
  downstream code that built `Job` instances directly (rare - the
  constructor is `pub(crate)`).
- **`events` cargo feature now pulls in `zbus`** on every target -
  unused on Windows, exercised on Linux/macOS. If you enable `events`
  in your dependency graph this adds ~1 MB to the dep tree.

### Removed (breaking)

- **`PrinterMonitor::list_printers`** - use `list_printers_cancellable(None)`.
- **`PrinterMonitor::find_printer`** - use `find_printer_cancellable(name, None)`.
  Both were deprecated in 1.5.0; the 2.0 bump removes the deprecation
  shim. The cancellable variants accept `None` for the existing
  no-cancellation behaviour.

### Migration

```rust
// 1.x
let printers = monitor.list_printers().await?;
let printer  = monitor.find_printer("HP").await?;

// 2.0
let printers = monitor.list_printers_cancellable(None).await?;
let printer  = monitor.find_printer_cancellable("HP", None).await?;
```

If your code matched exhaustively on `PrinterError`, `PrinterState`,
`ErrorState`, `PrinterStatus`, or `JobStatus`, add a `_ => ...` arm.

### Dependencies

- **Added** - `zbus` 5 (Linux D-Bus client, gated on the `events`
  feature), `futures-util` (signal stream iteration, same gate).

## [1.5.0] - 2026-05-17

### Added

#### Cargo Feature Flags
- **`rt-multi-thread` feature** (default-on) - tokio's multi-threaded runtime is now opt-in via this feature so library-only consumers can skip it. Existing users see no behaviour change because it ships enabled by default.
- **`serde` feature** - opt-in `Serialize`/`Deserialize` derives on public domain types (`Printer`, `PrinterStatus`, `PrinterState`, `ErrorState`, `PropertyChange`, `PrinterChanges`, `PrinterSummary`, `MonitorableProperty`, `Job`, `JobStatus`).
- **`tracing` feature** - opt-in routing of library log calls through the `tracing` crate instead of `log`.
- **`events` feature** (Windows only) - opt-in event-driven monitoring backed by WMI `__InstanceModificationEvent` subscriptions for sub-second printer-state change detection.

#### Fluent Monitor Builder
- **`PrinterMonitor::monitor(name) -> MonitorBuilder`** - chainable entry point for per-printer monitoring runs.
- **`MonitorBuilder` configuration** - `interval_ms`, `cancel_token`, `wait_for_appearance`, `filter_property`, `with_events`.
- **Terminal methods** - callback-based `run_changes`, `run_printer`, `run_property`; stream-based `run_changes_stream`, `run_property_stream` returning `tokio_stream::Stream`.

#### Cancellable Backend Queries
- **`PrinterMonitor::list_printers_cancellable(cancel)`** - cancellable listing.
- **`PrinterMonitor::find_printer_cancellable(name, cancel)`** - cancellable lookup.
- **`PrinterError::Cancelled`** variant - distinguishes a user-requested cancellation from a backend failure.

#### Print Job Tracking
- **`PrinterMonitor::list_jobs(printer_name, cancel)`** plus `Job` / `JobStatus` types - enumerate active print jobs cross-platform.
- **Windows** - reads `Win32_PrintJob` via WMI through a shared thread-local connection.
- **Linux** - parses `lpstat -l -o`; resolves `JobStatus` from `Status:` text and IPP `job-state-reasons` alerts.

#### Richer Linux State (M5)
- **`printer-state-reasons` parsing** - Linux backend now reads CUPS `Alerts:` continuation lines from `lpstat -l -p` and maps IPP reason tokens (`media-empty`, `toner-low`, `cover-open`, `media-jam`, `output-area-full`, `offline`, etc.) to typed `ErrorState` and `PrinterState` flag bits. Linux printers no longer surface as just `Idle / Printing / Offline`.

#### Linux Subprocess Hardening (B6 / B7 / F9 / F10)
- **Subprocess timeouts** - every `lpstat` invocation now runs inside `tokio::time::timeout` against a 5-second budget; a hung CUPS daemon surfaces as `PrinterError::CupsError` instead of blocking the monitor forever.
- **Locale-stable output** - all CUPS subprocesses spawn with `LANG=C` / `LC_ALL=C` so parsers never break under non-English user locales.

#### `PrinterError::TaskPanicked` (F6)
- **Typed panic surface** - `monitor_multiple_printers` now returns `PrinterError::TaskPanicked { printer_name, panic_message }` when a per-printer task panics, instead of stringifying the failure into `PrinterError::Other`. The panicking printer's name is preserved via `JoinSet::Id` correlation.

#### Miscellaneous
- **`PrinterMonitor::printer_summary_iter`** - iterator variant of `printer_summary` for callers that don't want a `HashMap` allocation.

### Changed
- **tokio runtime feature** - `rt-multi-thread` is now exposed as a same-named cargo feature on the library (default-on). The binary target keeps `required-features = ["rt-multi-thread"]` so it always has a runtime.
- **`PrinterState::from_u32`** is now cross-platform (was Windows-only); the Linux backend uses it to interpret bits derived from IPP `printer-state-reasons`. This is a pure logic change with no API impact.
- **Library log calls** route through an internal `pe_info!` / `pe_warn! `/ `pe_error!` facade that switches between `log::` and `tracing::` based on the `tracing` cargo feature.

### Deprecated
- **`PrinterMonitor::list_printers`** - use `list_printers_cancellable` instead. Removal planned for 2.0.
- **`PrinterMonitor::find_printer`** - use `find_printer_cancellable` instead. Removal planned for 2.0.

### Notes
- **Additive enum variants** - `PrinterError` gained `Cancelled` and `TaskPanicked` this release. The enum is not yet `#[non_exhaustive]` (planned for 2.0), so downstream code that `match`es `PrinterError` exhaustively without a wildcard arm will need a wildcard added when upgrading from 1.4.x.
- **No public API removals** - everything from 1.4.x continues to compile; only the two deprecation warnings above will fire on legacy call sites.

### Dependencies
- **tokio-stream 0.1** - added for the `Stream<Item = PrinterChanges>` / `Stream<Item = PropertyChange>` terminals.
- **tracing 0.1** (optional) - added behind the `tracing` cargo feature.

## [1.4.0] - 2025-01-06

### Added

#### Graceful Shutdown Support
- **CancellationToken support** - All monitoring functions now accept optional `CancellationToken` for graceful shutdown
- **Responsive cancellation** - Uses `tokio::select!` for immediate cancellation response during sleep intervals
- **Clean resource cleanup** - Monitoring tasks can be stopped gracefully without task abortion
- **Re-exported CancellationToken** - `tokio_util::sync::CancellationToken` now exported from main crate for convenience

#### Enhanced Monitor Architecture
- **Cloneable PrinterMonitor** - `PrinterMonitor` now implements `Clone` trait for efficient sharing
- **Arc-based backend** - Backend changed from `Box<dyn PrinterBackend>` to `Arc<dyn PrinterBackend>` for shared ownership
- **Efficient multi-printer monitoring** - Multiple monitors can now share the same backend connection
- **No duplicate connections** - Fixed issue where each monitored printer created a new backend instance

### Fixed

#### Code Quality Improvements
- **Edition compatibility** - Fixed Rust edition from non-existent "2024" to "2021"
- **Library logging** - Replaced `println!` with proper `log::info!()` macros in library code
- **PartialEq completeness** - Added missing `is_default` field to printer equality comparison
- **Mutex contention** - Fixed mutex contention by releasing locks before invoking callbacks

#### Testing Infrastructure
- **Comprehensive test coverage** - Added 52 tests covering all modules and platforms
- **Platform-specific tests** - Tests for both Windows (`#[cfg(windows)]`) and Unix (`#[cfg(unix)]`)
- **Cancellation token tests** - New tests for cancellation behavior and graceful shutdown
- **Clone verification** - Tests ensuring `PrinterMonitor` is properly cloneable

### Changed

#### API Updates - BREAKING CHANGE
- **Monitoring function signatures** - All monitoring functions now require `cancel_token: Option<CancellationToken>` parameter:
  - `monitor_printer(printer_name, interval_ms, cancel_token, callback)`
  - `monitor_printer_changes(printer_name, interval_ms, cancel_token, callback)`
  - `monitor_property(printer_name, property, interval_ms, cancel_token, callback)`
  - `monitor_multiple_printers(printer_names, interval_ms, cancel_token, callback)`
- **Pass None for no cancellation** - Existing code can pass `None` to maintain previous behavior

#### Behavioral Improvements
- **Initial state handling** - Callbacks no longer invoked on initial state capture to avoid empty changes
- **Lock-free callbacks** - Callbacks now execute outside of internal locks to prevent contention with slow user code

### Dependencies
- **tokio-util 0.7** - Added for `CancellationToken` support
- **tokio sync feature** - Added "sync" feature to tokio dependency

### Migration Guide

**Before (v1.3.2 and earlier):**
```rust
// No cancellation support
monitor.monitor_printer("Printer", 30000, |current, previous| {
    // callback logic
}).await?;
```

**After (v1.4.0+):**
```rust
use printer_event_handler::CancellationToken;

// With cancellation support
let cancel_token = CancellationToken::new();
let token_clone = cancel_token.clone();

let handle = tokio::spawn(async move {
    monitor.monitor_printer("Printer", 30000, Some(token_clone), |current, previous| {
        // callback logic
    }).await
});

// Later: cancel monitoring gracefully
cancel_token.cancel();
handle.await??;

// Or without cancellation (backward compatible)
monitor.monitor_printer("Printer", 30000, None, |current, previous| {
    // callback logic
}).await?;
```

**Cloneable Monitor:**
```rust
// Efficient sharing across tasks
let monitor = PrinterMonitor::new().await?;
let monitor_clone = monitor.clone(); // Cheap Arc clone

tokio::spawn(async move {
    monitor_clone.list_printers().await
});
```

### Technical Details
- **Performance** - No performance degradation; cancellation checks are lightweight
- **Memory efficiency** - Arc-based sharing reduces memory overhead for multi-printer monitoring
- **Backward compatibility** - Passing `None` for cancel_token maintains existing behavior
- **Cross-platform** - All improvements work identically on Windows and Linux

### Benefits
- **Production-ready** - Graceful shutdown enables proper service lifecycle management
- **Resource efficient** - Shared backend connections reduce system resource usage
- **Better testing** - Comprehensive test suite improves reliability and maintainability
- **Modern async patterns** - Follows Tokio best practices for cancellation and shutdown

## [1.3.2] - 2025-08-19

### Changed

#### Millisecond Precision Intervals - BREAKING CHANGE
- **Parameter change**: All monitoring functions now use milliseconds instead of seconds for interval parameters
- **Function signatures updated**: `interval_secs: u64` → `interval_ms: u64` in all monitoring methods:
  - `monitor_printer(printer_name, interval_ms, callback)`
  - `monitor_printer_changes(printer_name, interval_ms, callback)`
  - `monitor_property(printer_name, property, interval_ms, callback)`
  - `monitor_multiple_printers(printer_names, interval_ms, callback)`

### Added

#### Enhanced Monitoring Precision
- **Sub-second monitoring**: Can now monitor printer changes at intervals less than 1 second
- **Millisecond granularity**: Precise control over monitoring frequency (100ms, 500ms, etc.)
- **Common interval examples**: Documentation includes common millisecond values for different use cases
- **High-frequency monitoring**: Support for rapid change detection in time-sensitive applications

### Updated

#### Documentation and Examples
- **README.md**: Updated all examples to use millisecond values (30000ms instead of 30s)
- **Code examples**: All monitoring examples updated with millisecond intervals
- **CLI application**: Updated to use 60000ms (60 seconds) instead of 60s
- **Library documentation**: Updated inline documentation and code examples
- **Examples directory**: All example files updated to use millisecond precision

### Migration Guide

**Before (v1.3.1 and earlier - seconds):**
```rust
// 30 seconds interval (using seconds)
monitor.monitor_printer("Printer", 30, callback).await?;
monitor.monitor_property("Printer", property, 60, callback).await?;
```

**After (v1.3.2+ - milliseconds):**
```rust
// 30 seconds = 30000 milliseconds
monitor.monitor_printer("Printer", 30000, callback).await?;
monitor.monitor_property("Printer", property, 60000, callback).await?;

// New precision capabilities:
monitor.monitor_printer("Printer", 500, callback).await?;  // 0.5 seconds
monitor.monitor_printer("Printer", 100, callback).await?;  // 0.1 seconds
```

**Migration Steps:**
1. Multiply all existing interval values by 1000
2. Consider if you need higher precision monitoring (sub-second intervals)
3. Update any hardcoded interval values in your code

### Technical Details
- **Performance**: No performance impact - same underlying `Duration::from_millis()` usage
- **Precision**: Can now handle intervals as low as 1ms (though not recommended for printer monitoring)
- **Backward compatibility**: Breaking change - requires code updates for interval values
- **Cross-platform**: Works identically on Windows and Linux

### Benefits
- **Fine-grained control**: Monitor rapid printer state changes in real-time applications
- **Flexible intervals**: Choose optimal monitoring frequency for different use cases
- **Better responsiveness**: Detect printer changes faster when needed
- **Production flexibility**: Adjust monitoring precision based on system requirements

## [1.3.1] - 2025-08-19

### Added

#### Type-Safe Property Monitoring
- **`MonitorableProperty` enum** - New strongly-typed enum for specifying properties to monitor
- **Type-safe `monitor_property()` API** - Replaces string-based property names with enum variants
- **Complete property coverage** - All 12 monitorable printer properties available as enum variants:
  - `Name` - Printer name changes
  - `Status` - PrinterStatus enum changes (recommended)
  - `State` - PrinterState enum changes (legacy Windows)
  - `ErrorState` - ErrorState enum changes
  - `IsOffline` - Online/offline status changes
  - `IsDefault` - Default printer designation changes
  - `PrinterStatusCode` - Raw PrinterStatus code changes (1-7)
  - `PrinterStateCode` - Raw PrinterState code changes (.NET flags)
  - `DetectedErrorStateCode` - Raw DetectedErrorState code changes (0-11)
  - `ExtendedDetectedErrorStateCode` - Raw ExtendedDetectedErrorState code changes
  - `ExtendedPrinterStatusCode` - Raw ExtendedPrinterStatus code changes
  - `WmiStatus` - WMI Status property changes

### Enhanced

#### Developer Experience
- **IDE auto-completion** - MonitorableProperty enum provides IntelliSense support
- **Compile-time validation** - Invalid property names caught at compile time instead of runtime
- **Self-documenting code** - Each enum variant includes descriptive documentation
- **Helper methods** - `as_str()`, `description()`, and `all()` methods for convenience

#### API Improvements
- **Backward compatible** - Internal string conversion maintains existing functionality
- **Future-proof design** - Easy to add new properties while maintaining API stability
- **Better error prevention** - Eliminates typos in property name strings

### Updated

#### Documentation
- **README.md** - Added complete MonitorableProperty documentation with usage examples
- **Library documentation** - Updated main example to showcase type-safe property monitoring
- **Examples** - Updated `property_monitoring.rs` to use new enum-based API
- **API Reference** - Added MonitorableProperty to core types documentation

#### Examples
- **`property_monitoring.rs`** - Now demonstrates type-safe property selection
- **Type-safe examples** - Shows usage of `MonitorableProperty::IsOffline` and `MonitorableProperty::Status`

### Migration Guide

**Before (v1.3.0 and earlier):**
```rust
monitor.monitor_property("Printer", "IsOffline", 60, |change| {
    println!("Change: {}", change.description());
}).await?;
```

**After (v1.3.1+):**
```rust
use printer_event_handler::MonitorableProperty;

monitor.monitor_property("Printer", MonitorableProperty::IsOffline, 60, |change| {
    println!("Change: {}", change.description());
}).await?;
```

### Technical Details
- **No breaking changes** - Existing string-based usage still works internally
- **Performance** - No runtime overhead, enum converts to string internally
- **Compile-time safety** - Invalid properties caught during compilation
- **Cross-platform** - Works identically on Windows and Linux

## [1.3.0] - 2025-01-18

### Fixed

#### PrinterState Accuracy - BREAKING INTERNAL CHANGE
- **Correct WMI Value Mapping** - Fixed critical issue where PrinterState enum used incorrect 0-25 range instead of actual WMI values
- **Real WMI Values** - PrinterState now correctly handles actual WMI values like 1024 (Printing), 16384 (Processing), 128 (Offline)
- **.NET PrintQueueStatus Implementation** - Replaced PrinterState with proper .NET PrintQueueStatus flag-based system
- **Bitwise Flag Support** - PrinterState now properly handles multiple concurrent states using bitwise flags
- **Priority-Based Selection** - When multiple flags are set, selects the most significant state (errors → active → idle)

#### Enhanced Property Monitoring
- **Property Change Detection** - Added `PropertyChange` enum for tracking individual property changes
- **Detailed Monitoring** - New `monitor_printer_changes()` method for granular change detection  
- **Property-Specific Monitoring** - New `monitor_property()` method for watching individual properties
- **Multi-Printer Monitoring** - New `monitor_multiple_printers()` method for concurrent monitoring
- **Property Change Collection** - New `PrinterChanges` struct for organizing detected changes

### Added

#### .NET PrintQueueStatus Flag Values
- **Complete Flag Coverage** - All 23 PrintQueueStatus flag values now supported:
  - `None` (0), `Paused` (1), `Error` (2), `PendingDeletion` (4)
  - `PaperJam` (8), `PaperOut` (16), `ManualFeed` (32), `PaperProblem` (64)
  - `Offline` (128), `IOActive` (256), `Busy` (512), `Printing` (1024)
  - `OutputBinFull` (2048), `NotAvailable` (4096), `Waiting` (8192)
  - `Processing` (16384), `Initializing` (32768), `WarmingUp` (65536)
  - `TonerLow` (131072), `NoToner` (262144), `PagePunt` (524288)
  - `UserInterventionRequired` (1048576), `OutOfMemory` (2097152)
  - `DoorOpen` (4194304), `ServerUnknown` (8388608), `PowerSave` (16777216)

#### Enhanced Status Detection
- **Smart Flag Interpretation** - Prioritized flag parsing (errors first, then active states, then idle)
- **Status Conversion Methods** - `is_error()` and `is_offline()` methods for PrinterState
- **PrinterStatus Mapping** - Improved `to_printer_status()` conversion for compatibility

### Enhanced
- **API Compatibility** - No breaking changes to public API despite internal restructure
- **Accurate Status Display** - CLI now shows correct status for real printer values
- **Comprehensive Examples** - New property_monitoring.rs example demonstrating advanced features
- **Documentation** - Updated README with accurate PrinterState flag values and examples

### Technical Details
- **Microsoft Documentation Compliance** - Now follows [.NET PrintQueueStatus](https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus) specification
- **Backward Compatible** - All existing API methods work unchanged
- **Performance** - Enhanced priority-based flag parsing for efficient status determination
- **Testing** - All library tests updated and pass with new implementation

## [1.2.0] - 2025-01-18

### Added

#### Complete WMI Property Access
- **Raw Status Code Getters** - Access all numeric WMI status codes:
  - `printer_status_code()` → PrinterStatus (1-7, current/recommended property)
  - `printer_state_code()` → PrinterState (.NET PrintQueueStatus flags)
  - `detected_error_state_code()` → DetectedErrorState (0-11)
  - `extended_printer_status_code()` → ExtendedPrinterStatus 
  - `extended_detected_error_state_code()` → ExtendedDetectedErrorState
  - `wmi_status()` → Status property string ("OK", "Degraded", "Error", etc.)

#### Human-Readable Description Methods
- **Status Code Descriptions** - Get human-readable descriptions for all status codes:
  - `printer_status_description()` → "Idle", "Printing", "Other", etc.
  - `printer_state_description()` → "Paper Jam", "Toner Low", "Busy", etc.
  - `detected_error_state_description()` → "No Paper", "Jammed", etc.
  - `extended_printer_status_description()` → Extended status descriptions

#### Enhanced WMI Integration
- **Additional WMI Properties** - Now queries ExtendedDetectedErrorState and Status properties
- **Complete Property Storage** - Printer struct stores all raw WMI status codes
- **Comprehensive Constructor** - New `new_with_wmi()` method for complete WMI data

#### Improved Offline Detection
- **ExtendedPrinterStatus Support** - Uses ExtendedPrinterStatus=7 for offline detection
- **WMI Status Integration** - Uses Status property ("Degraded", "Error", etc.) for offline detection
- **Multi-Property Logic** - Enhanced offline detection using all available WMI properties

### Enhanced
- **Comprehensive WMI Coverage** - Complete access to all Win32_Printer properties
- **Debug Information** - CLI now displays detailed WMI information for analysis
- **Documentation** - Updated all documentation with new getter methods and examples

### Technical Details
- **API Expansion** - Added 10+ new getter methods for complete WMI property access
- **Backward Compatible** - All existing API methods preserved
- **Performance** - Single WMI query retrieves all properties efficiently

## [1.1.0] - 2025-01-17

### Added

#### Comprehensive Win32_Printer Support  
- **Complete PrinterState mapping** - Added support for 26 PrinterState values (0-25) based on Win32_Printer documentation:
  - `Paused` (1) - Printer paused
  - `Error` (2) - General error state
  - `PendingDeletion` (3) - Queued for deletion
  - `PaperJam` (4) - Paper jam detected
  - `PaperOut` (5) - Out of paper
  - `ManualFeed` (6) - Manual feed required
  - `PaperProblem` (7) - Paper-related issue
  - `IOActive` (9) - I/O operations active
  - `Busy` (10) - Printer busy
  - `OutputBinFull` (12) - Output tray full
  - `NotAvailable` (13) - Printer not available
  - `Waiting` (14) - Waiting for job
  - `Processing` (15) - Processing job
  - `Initialization` (16) - Initializing
  - `TonerLow` (18) - Low toner/ink
  - `NoToner` (19) - Out of toner/ink
  - `PagePunt` (20) - Page punt condition
  - `UserInterventionRequired` (21) - User action needed
  - `OutOfMemory` (22) - Memory full
  - `DoorOpen` (23) - Cover/door open
  - `ServerUnknown` (24) - Server status unknown
  - `PowerSave` (25) - Power save mode

#### Enhanced Error State Detection
- **Complete DetectedErrorState mapping** - Now supports all 12 DetectedErrorState values (0-11)
- **Fixed value 0 mapping** - Correctly maps DetectedErrorState=0 to `NoError` for better real-world compatibility
- **Added OutputBinFull support** - DetectedErrorState=11 now properly mapped

#### Improved Offline Detection
- **Enhanced offline logic** - Printers with status `Error`, `NotAvailable`, or `ServerUnknown` are now correctly marked as offline
- **Consistent status reporting** - Fixed issue where printers showed `Status: Offline` but `Offline: No`

### Fixed
- **PrinterState value 0** - Now correctly maps to `Idle` instead of falling through to `StatusUnknown`
- **PrinterState value 128** - Legacy offline state properly handled for backward compatibility
- **DetectedErrorState practical mapping** - Value 0 now maps to `NoError` instead of `UnknownError` for better user experience

### Enhanced
- **Fallback logic** - Improved PrinterState/PrinterStatus fallback when one field has unmapped values
- **Documentation** - Complete coverage of all supported status values in README.md and API docs
- **Cross-platform compatibility** - Linux backend remains fully functional with new Windows enum variants

### Technical Details
- **Microsoft Win32_Printer compliance** - Implementation based on [official documentation](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printer)
- **No breaking changes** - All existing enum variants preserved, only additions made
- **Status mapping validation** - Enum values tested against available printer data

**Note**: The 0-25 range mapping was later discovered to be incorrect in v1.3.0. Actual WMI values use .NET PrintQueueStatus flags.

## [1.0.0] - 2024-XX-XX

### Added
- Initial release
- Cross-platform printer monitoring (Windows WMI, Linux CUPS)
- Basic printer status detection
- Async/await support with Tokio
- CLI interface for printer monitoring
- Core PrinterStatus and ErrorState enums
- Real-time printer monitoring capabilities

### Features
- Windows WMI backend for printer detection
- Linux CUPS backend with lpstat integration
- Printer discovery and enumeration
- Status change monitoring with callbacks
- Error state detection and reporting
- Cross-platform API compatibility