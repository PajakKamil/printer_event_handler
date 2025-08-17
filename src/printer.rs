#[cfg(windows)]
use serde::Deserialize;

/// Represents a printer's status
#[derive(Debug, Clone, PartialEq)]
pub enum PrinterStatus {
    // PrinterStatus values (1-7)
    Other,
    Unknown,
    Idle,
    Printing,
    Warmup,
    StoppedPrinting,
    Offline,

    // Additional PrinterState values (0-25)
    Paused,
    Error,
    PendingDeletion,
    PaperJam,
    PaperOut,
    ManualFeed,
    PaperProblem,
    IOActive,
    Busy,
    OutputBinFull,
    NotAvailable,
    Waiting,
    Processing,
    Initialization,
    TonerLow,
    NoToner,
    PagePunt,
    UserInterventionRequired,
    OutOfMemory,
    DoorOpen,
    ServerUnknown,
    PowerSave,

    // Fallback for unmapped values
    StatusUnknown,
}

impl PrinterStatus {
    /// Creates a PrinterStatus from a WMI status code.
    ///
    /// # Arguments
    /// * `status` - Optional WMI printer status code
    ///
    /// # Returns
    /// Corresponding PrinterStatus enum variant
    #[cfg(windows)]
    pub(crate) fn from_u32(status: Option<u32>) -> Self {
        match status {
            Some(1) => PrinterStatus::Other,
            Some(2) => PrinterStatus::Unknown,
            Some(3) => PrinterStatus::Idle,
            Some(4) => PrinterStatus::Printing,
            Some(5) => PrinterStatus::Warmup,
            Some(6) => PrinterStatus::StoppedPrinting,
            Some(7) => PrinterStatus::Offline,
            _ => PrinterStatus::StatusUnknown,
        }
    }

    /// Creates a PrinterStatus from a WMI PrinterState value.
    ///
    /// # Arguments
    /// * `state` - WMI Win32_Printer.PrinterState value (0-25 according to documentation)
    ///
    /// # Returns
    /// Corresponding PrinterStatus enum variant
    #[cfg(windows)]
    pub(crate) fn from_printer_state(state: u32) -> Self {
        // PrinterState values from WMI Win32_Printer.PrinterState documentation
        match state {
            0 => PrinterStatus::Idle,                      // Idle
            1 => PrinterStatus::Paused,                    // Paused
            2 => PrinterStatus::Error,                     // Error
            3 => PrinterStatus::PendingDeletion,           // Pending Deletion
            4 => PrinterStatus::PaperJam,                  // Paper Jam
            5 => PrinterStatus::PaperOut,                  // Paper Out
            6 => PrinterStatus::ManualFeed,                // Manual Feed
            7 => PrinterStatus::PaperProblem,              // Paper Problem
            8 => PrinterStatus::Offline,                   // Offline
            9 => PrinterStatus::IOActive,                  // I/O Active
            10 => PrinterStatus::Busy,                     // Busy
            11 => PrinterStatus::Printing,                 // Printing
            12 => PrinterStatus::OutputBinFull,            // Output Bin Full
            13 => PrinterStatus::NotAvailable,             // Not Available
            14 => PrinterStatus::Waiting,                  // Waiting
            15 => PrinterStatus::Processing,               // Processing
            16 => PrinterStatus::Initialization,           // Initialization
            17 => PrinterStatus::Warmup,                   // Warming Up
            18 => PrinterStatus::TonerLow,                 // Toner Low
            19 => PrinterStatus::NoToner,                  // No Toner
            20 => PrinterStatus::PagePunt,                 // Page Punt
            21 => PrinterStatus::UserInterventionRequired, // User Intervention Required
            22 => PrinterStatus::OutOfMemory,              // Out of Memory
            23 => PrinterStatus::DoorOpen,                 // Door Open
            24 => PrinterStatus::ServerUnknown,            // Server Unknown
            25 => PrinterStatus::PowerSave,                // Power Save
            128 => PrinterStatus::Offline,                 // Legacy offline state
            _ => PrinterStatus::StatusUnknown,             // Unmapped values
        }
    }

    /// Returns a human-readable description of this printer status.
    ///
    /// # Returns
    /// A static string describing the status (e.g., "Idle", "Printing", "Offline")
    ///
    /// # Example
    /// ```
    /// use printer_event_handler::PrinterStatus;
    ///
    /// let status = PrinterStatus::Printing;
    /// assert_eq!(status.description(), "Printing");
    /// ```
    pub fn description(&self) -> &'static str {
        match self {
            // PrinterStatus values (1-7)
            PrinterStatus::Other => "Other",
            PrinterStatus::Unknown => "Unknown",
            PrinterStatus::Idle => "Idle",
            PrinterStatus::Printing => "Printing",
            PrinterStatus::Warmup => "Warming Up",
            PrinterStatus::StoppedPrinting => "Stopped Printing",
            PrinterStatus::Offline => "Offline",

            // Additional PrinterState values (0-25)
            PrinterStatus::Paused => "Paused",
            PrinterStatus::Error => "Error",
            PrinterStatus::PendingDeletion => "Pending Deletion",
            PrinterStatus::PaperJam => "Paper Jam",
            PrinterStatus::PaperOut => "Paper Out",
            PrinterStatus::ManualFeed => "Manual Feed",
            PrinterStatus::PaperProblem => "Paper Problem",
            PrinterStatus::IOActive => "I/O Active",
            PrinterStatus::Busy => "Busy",
            PrinterStatus::OutputBinFull => "Output Bin Full",
            PrinterStatus::NotAvailable => "Not Available",
            PrinterStatus::Waiting => "Waiting",
            PrinterStatus::Processing => "Processing",
            PrinterStatus::Initialization => "Initialization",
            PrinterStatus::TonerLow => "Toner Low",
            PrinterStatus::NoToner => "No Toner",
            PrinterStatus::PagePunt => "Page Punt",
            PrinterStatus::UserInterventionRequired => "User Intervention Required",
            PrinterStatus::OutOfMemory => "Out of Memory",
            PrinterStatus::DoorOpen => "Door Open",
            PrinterStatus::ServerUnknown => "Server Unknown",
            PrinterStatus::PowerSave => "Power Save",

            // Fallback
            PrinterStatus::StatusUnknown => "Status Unknown",
        }
    }
}

impl std::fmt::Display for PrinterStatus {
    /// Formats the PrinterStatus for display.
    ///
    /// # Arguments
    /// * `f` - Formatter instance
    ///
    /// # Returns
    /// Result of the formatting operation
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Represents a printer's error state
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorState {
    NoError,
    Other,
    LowPaper,
    NoPaper,
    LowToner,
    NoToner,
    DoorOpen,
    Jammed,
    ServiceRequested,
    OutputBinFull,
    UnknownError,
}

impl ErrorState {
    /// Creates an ErrorState from a WMI error code.
    ///
    /// # Arguments
    /// * `error` - Optional WMI detected error state code
    ///
    /// # Returns
    /// Corresponding ErrorState enum variant
    #[cfg(windows)]
    pub(crate) fn from_u32(error: Option<u32>) -> Self {
        match error {
            // Note: In practice, many printers report 0 when working normally,
            // despite documentation saying 0=Unknown. We map 0 to NoError for better UX.
            Some(0) => ErrorState::NoError, // Unknown (but often means no error in practice)
            Some(1) => ErrorState::Other,   // Other
            Some(2) => ErrorState::NoError, // No Error
            Some(3) => ErrorState::LowPaper, // Low Paper
            Some(4) => ErrorState::NoPaper, // No Paper
            Some(5) => ErrorState::LowToner, // Low Toner
            Some(6) => ErrorState::NoToner, // No Toner
            Some(7) => ErrorState::DoorOpen, // Door Open
            Some(8) => ErrorState::Jammed,  // Jammed
            Some(9) => ErrorState::Other, // Offline (map to Other since we have separate offline status)
            Some(10) => ErrorState::ServiceRequested, // Service Requested
            Some(11) => ErrorState::OutputBinFull, // Output Bin Full
            _ => ErrorState::UnknownError, // Unmapped values
        }
    }

    /// Returns a human-readable description of this error state.
    ///
    /// # Returns
    /// A static string describing the error condition
    ///
    /// # Example
    /// ```
    /// use printer_event_handler::ErrorState;
    ///
    /// let error = ErrorState::NoPaper;
    /// assert_eq!(error.description(), "No Paper");
    /// ```
    pub fn description(&self) -> &'static str {
        match self {
            ErrorState::NoError => "No Error",
            ErrorState::Other => "Other",
            ErrorState::LowPaper => "Low Paper",
            ErrorState::NoPaper => "No Paper",
            ErrorState::LowToner => "Low Toner",
            ErrorState::NoToner => "No Toner",
            ErrorState::DoorOpen => "Door Open",
            ErrorState::Jammed => "Jammed",
            ErrorState::ServiceRequested => "Service Requested",
            ErrorState::OutputBinFull => "Output Bin Full",
            ErrorState::UnknownError => "Unknown Error State",
        }
    }

    /// Determines whether this error state represents an actual error condition.
    ///
    /// # Returns
    /// `true` if this represents an error that needs attention, `false` for normal operation
    ///
    /// # Example
    /// ```
    /// use printer_event_handler::ErrorState;
    ///
    /// assert!(!ErrorState::NoError.is_error());
    /// assert!(ErrorState::Jammed.is_error());
    /// ```
    pub fn is_error(&self) -> bool {
        !matches!(self, ErrorState::NoError)
    }
}

impl std::fmt::Display for ErrorState {
    /// Formats the ErrorState for display.
    ///
    /// # Arguments
    /// * `f` - Formatter instance
    ///
    /// # Returns
    /// Result of the formatting operation
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Internal WMI printer representation
#[cfg(windows)]
#[derive(Deserialize, Debug)]
pub(crate) struct Win32Printer {
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "PrinterStatus")]
    pub printer_status: Option<u32>,
    #[serde(rename = "DetectedErrorState")]
    pub detected_error_state: Option<u32>,
    #[serde(rename = "WorkOffline")]
    pub work_offline: Option<bool>,
    #[serde(rename = "PrinterState")]
    pub printer_state: Option<u32>,
    #[serde(rename = "Default")]
    pub default: Option<bool>,
}

/// Represents a printer and its current state
#[derive(Debug, Clone)]
pub struct Printer {
    name: String,
    status: PrinterStatus,
    error_state: ErrorState,
    is_offline: bool,
    is_default: bool,
}

impl Printer {
    /// Creates a new Printer instance with the specified properties.
    ///
    /// # Arguments
    /// * `name` - The printer's name as it appears in the system
    /// * `status` - Current operational status of the printer
    /// * `error_state` - Current error condition, if any
    /// * `is_offline` - Whether the printer is currently offline
    /// * `is_default` - Whether this is the system's default printer
    ///
    /// # Returns
    /// A new Printer instance with the specified properties
    ///
    /// # Example
    /// ```
    /// use printer_event_handler::{Printer, PrinterStatus, ErrorState};
    ///
    /// let printer = Printer::new(
    ///     "My Printer".to_string(),
    ///     PrinterStatus::Idle,
    ///     ErrorState::NoError,
    ///     false,
    ///     true,
    /// );
    /// ```
    pub fn new(
        name: String,
        status: PrinterStatus,
        error_state: ErrorState,
        is_offline: bool,
        is_default: bool,
    ) -> Self {
        Self {
            name,
            status,
            error_state,
            is_offline,
            is_default,
        }
    }

    /// Returns the printer's name as registered in the system.
    ///
    /// # Returns
    /// A string slice containing the printer's name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the printer's current operational status.
    ///
    /// # Returns
    /// Reference to the printer's PrinterStatus
    pub fn status(&self) -> &PrinterStatus {
        &self.status
    }

    /// Returns a human-readable description of the printer's current status.
    ///
    /// # Returns
    /// A static string describing the printer's status (e.g., "Idle", "Printing")
    pub fn status_description(&self) -> &'static str {
        self.status.description()
    }

    /// Returns a reference to the printer's current error state.
    ///
    /// # Returns
    /// Reference to the printer's ErrorState
    pub fn error_state(&self) -> &ErrorState {
        &self.error_state
    }

    /// Returns a human-readable description of the printer's current error state.
    ///
    /// # Returns
    /// A static string describing the error state (e.g., "No Error", "Paper Jam")
    pub fn error_description(&self) -> &'static str {
        self.error_state.description()
    }

    /// Checks whether the printer is currently offline or disconnected.
    ///
    /// # Returns
    /// `true` if the printer is offline, `false` if it's online and available
    pub fn is_offline(&self) -> bool {
        self.is_offline
    }

    /// Checks whether this printer is set as the system's default printer.
    ///
    /// # Returns
    /// `true` if this is the default printer, `false` otherwise
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Checks whether the printer currently has any error conditions.
    ///
    /// This is a convenience method that checks if the error state indicates
    /// any kind of problem with the printer.
    ///
    /// # Returns
    /// `true` if the printer has an error condition, `false` if operating normally
    pub fn has_error(&self) -> bool {
        self.error_state.is_error()
    }
}

#[cfg(windows)]
impl From<Win32Printer> for Printer {
    /// Converts a WMI Win32_Printer object into a Printer instance.
    ///
    /// This implementation handles the conversion from Windows WMI printer data
    /// to our unified Printer representation, mapping WMI-specific fields to
    /// our cross-platform enum types.
    ///
    /// # Arguments
    /// * `wmi_printer` - WMI printer data from Win32_Printer query
    ///
    /// # Returns
    /// A Printer instance with data converted from WMI format
    fn from(wmi_printer: Win32Printer) -> Self {
        // Use PrinterState for more accurate status determination if available and valid,
        // otherwise fall back to PrinterStatus
        let status = if let Some(printer_state) = wmi_printer.printer_state {
            let state_status = PrinterStatus::from_printer_state(printer_state);
            // If PrinterState gives us an unknown status, try PrinterStatus as fallback
            if matches!(state_status, PrinterStatus::StatusUnknown) {
                PrinterStatus::from_u32(wmi_printer.printer_status)
            } else {
                state_status
            }
        } else {
            PrinterStatus::from_u32(wmi_printer.printer_status)
        };

        // Determine offline status from both WorkOffline field and printer status
        let is_offline = wmi_printer.work_offline.unwrap_or(false)
            || matches!(
                status,
                PrinterStatus::Offline
                    | PrinterStatus::Error
                    | PrinterStatus::NotAvailable
                    | PrinterStatus::ServerUnknown
            );

        Self {
            name: wmi_printer
                .name
                .unwrap_or_else(|| "Unknown Printer".to_string()),
            status,
            error_state: ErrorState::from_u32(wmi_printer.detected_error_state),
            is_offline,
            is_default: wmi_printer.default.unwrap_or(false),
        }
    }
}

impl PartialEq for Printer {
    /// Compares two Printer instances for equality.
    ///
    /// Two printers are considered equal if they have the same name, status,
    /// error state, and offline status. The default printer flag is not
    /// considered in equality comparison.
    ///
    /// # Arguments
    /// * `other` - The other Printer to compare against
    ///
    /// # Returns
    /// `true` if the printers are equivalent, `false` otherwise
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.status == other.status
            && self.error_state == other.error_state
            && self.is_offline == other.is_offline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_status_display() {
        assert_eq!(PrinterStatus::Idle.to_string(), "Idle");
        assert_eq!(PrinterStatus::Printing.to_string(), "Printing");
    }

    #[test]
    fn test_error_state_is_error() {
        assert!(!ErrorState::NoError.is_error());
        assert!(ErrorState::Jammed.is_error());
        assert!(ErrorState::NoPaper.is_error());
    }

    #[test]
    fn test_printer_creation() {
        let printer = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        assert_eq!(printer.name(), "Test Printer");
        assert_eq!(printer.status(), &PrinterStatus::Idle);
        assert!(!printer.has_error());
        assert!(printer.is_default());
        assert!(!printer.is_offline());
    }
}
