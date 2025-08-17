
#[cfg(windows)]
use serde::Deserialize;

/// Represents a printer's status
#[derive(Debug, Clone, PartialEq)]
pub enum PrinterStatus {
    Other,
    Unknown,
    Idle,
    Printing,
    Warmup,
    StoppedPrinting,
    Offline,
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
    /// * `state` - WMI Win32_Printer.PrinterState value
    ///
    /// # Returns
    /// Corresponding PrinterStatus enum variant
    #[cfg(windows)]
    pub(crate) fn from_printer_state(state: u32) -> Self {
        // PrinterState values from WMI Win32_Printer.PrinterState
        match state {
            0 => PrinterStatus::Idle, // 0 = Ready/Normal state
            1 => PrinterStatus::Other,
            2 => PrinterStatus::Unknown, 
            3 => PrinterStatus::Idle,
            4 => PrinterStatus::Printing,
            5 => PrinterStatus::Warmup,
            6 => PrinterStatus::StoppedPrinting,
            7 => PrinterStatus::Offline,
            128 => PrinterStatus::Offline, // 128 = Offline state
            _ => PrinterStatus::StatusUnknown,
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
            PrinterStatus::Other => "Other",
            PrinterStatus::Unknown => "Unknown",
            PrinterStatus::Idle => "Idle",
            PrinterStatus::Printing => "Printing",
            PrinterStatus::Warmup => "Warmup",
            PrinterStatus::StoppedPrinting => "Stopped Printing",
            PrinterStatus::Offline => "Offline",
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
            Some(0) | Some(2) => ErrorState::NoError,
            Some(1) => ErrorState::Other,
            Some(3) => ErrorState::LowPaper,
            Some(4) => ErrorState::NoPaper,
            Some(5) => ErrorState::LowToner,
            Some(6) => ErrorState::NoToner,
            Some(7) => ErrorState::DoorOpen,
            Some(8) => ErrorState::Jammed,
            Some(9) => ErrorState::ServiceRequested,
            Some(10) => ErrorState::OutputBinFull,
            _ => ErrorState::UnknownError,
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
            || matches!(status, PrinterStatus::Offline);

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
