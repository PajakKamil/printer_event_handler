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
    fn from_u32(status: Option<u32>) -> Self {
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

    fn from_printer_state(state: u32) -> Self {
        // PrinterState values from WMI Win32_Printer.PrinterState
        match state {
            1 => PrinterStatus::Other,
            2 => PrinterStatus::Unknown, 
            3 => PrinterStatus::Idle,
            4 => PrinterStatus::Printing,
            5 => PrinterStatus::Warmup,
            6 => PrinterStatus::StoppedPrinting,
            7 => PrinterStatus::Offline,
            _ => PrinterStatus::StatusUnknown,
        }
    }

    /// Get a human-readable description of the printer status
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
    fn from_u32(error: Option<u32>) -> Self {
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

    /// Get a human-readable description of the error state
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

    /// Check if this represents an error condition
    pub fn is_error(&self) -> bool {
        !matches!(self, ErrorState::NoError)
    }
}

impl std::fmt::Display for ErrorState {
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
    /// Create a new Printer instance
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

    /// Get the printer's name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the printer's current status
    pub fn status(&self) -> &PrinterStatus {
        &self.status
    }

    /// Get a human-readable description of the printer's status
    pub fn status_description(&self) -> &'static str {
        self.status.description()
    }

    /// Get the printer's error state
    pub fn error_state(&self) -> &ErrorState {
        &self.error_state
    }

    /// Get a human-readable description of the printer's error state
    pub fn error_description(&self) -> &'static str {
        self.error_state.description()
    }

    /// Check if the printer is currently offline
    pub fn is_offline(&self) -> bool {
        self.is_offline
    }

    /// Check if this is the default printer
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Check if the printer has any error conditions
    pub fn has_error(&self) -> bool {
        self.error_state.is_error()
    }
}

#[cfg(windows)]
impl From<Win32Printer> for Printer {
    fn from(wmi_printer: Win32Printer) -> Self {
        // Use PrinterState for more accurate status determination if available
        let status = if let Some(printer_state) = wmi_printer.printer_state {
            PrinterStatus::from_printer_state(printer_state)
        } else {
            PrinterStatus::from_u32(wmi_printer.printer_status)
        };

        Self {
            name: wmi_printer
                .name
                .unwrap_or_else(|| "Unknown Printer".to_string()),
            status,
            error_state: ErrorState::from_u32(wmi_printer.detected_error_state),
            is_offline: wmi_printer.work_offline.unwrap_or(false),
            is_default: wmi_printer.default.unwrap_or(false),
        }
    }
}

impl PartialEq for Printer {
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
