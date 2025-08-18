# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2025-01-18

### Added

#### Complete WMI Property Access
- **Raw Status Code Getters** - Access all numeric WMI status codes:
  - `printer_status_code()` → PrinterStatus (1-7, current/recommended property)
  - `printer_state_code()` → PrinterState (0-25, obsolete property)
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
- **Complete PrinterState mapping** - Now supports all 26 PrinterState values (0-25) according to Microsoft Win32_Printer documentation:
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
- **Microsoft Win32_Printer compliance** - Full implementation according to [official documentation](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printer)
- **No breaking changes** - All existing enum variants preserved, only additions made
- **Comprehensive testing** - All status mappings validated against real Windows printer data

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