# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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