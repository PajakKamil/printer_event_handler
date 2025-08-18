#[cfg(windows)]
use serde::Deserialize;

/// Represents a printer's status (Win32_Printer.PrinterStatus - Current/Recommended)
/// 
/// This is the current WMI property for printer status information.
/// Values 1-7 according to Microsoft documentation.
#[derive(Debug, Clone, PartialEq)]
pub enum PrinterStatus {
    Other,           // 1
    Unknown,         // 2
    Idle,            // 3
    Printing,        // 4
    Warmup,          // 5
    StoppedPrinting, // 6
    Offline,         // 7
    StatusUnknown,   // Fallback for unmapped values
}

/// Represents a printer's state (Win32_Printer.PrinterState - Obsolete/Deprecated)
/// 
/// This is the obsolete WMI property. Use PrinterStatus when available.
/// Values 0-25 according to Microsoft documentation.
#[derive(Debug, Clone, PartialEq)]
pub enum PrinterState {
    Idle,                        // 0
    Paused,                      // 1
    Error,                       // 2
    PendingDeletion,             // 3
    PaperJam,                    // 4
    PaperOut,                    // 5
    ManualFeed,                  // 6
    PaperProblem,                // 7
    Offline,                     // 8
    IOActive,                    // 9
    Busy,                        // 10
    Printing,                    // 11
    OutputBinFull,               // 12
    NotAvailable,                // 13
    Waiting,                     // 14
    Processing,                  // 15
    Initialization,              // 16
    Warmup,                      // 17
    TonerLow,                    // 18
    NoToner,                     // 19
    PagePunt,                    // 20
    UserInterventionRequired,    // 21
    OutOfMemory,                 // 22
    DoorOpen,                    // 23
    ServerUnknown,               // 24
    PowerSave,                   // 25
    StateUnknown,                // Fallback for unmapped values
}

impl PrinterStatus {
    /// Creates a PrinterStatus from a WMI status code.
    ///
    /// # Arguments
    /// * `status` - Optional WMI printer status code (1-7)
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
            PrinterStatus::Warmup => "Warming Up",
            PrinterStatus::StoppedPrinting => "Stopped Printing",
            PrinterStatus::Offline => "Offline",
            PrinterStatus::StatusUnknown => "Status Unknown",
        }
    }
}

impl PrinterState {
    /// Creates a PrinterState from a WMI PrinterState value (obsolete property).
    ///
    /// # Arguments
    /// * `state` - WMI Win32_Printer.PrinterState value (0-25)
    ///
    /// # Returns
    /// Corresponding PrinterState enum variant
    #[cfg(windows)]
    pub(crate) fn from_u32(state: u32) -> Self {
        // PrinterState values from WMI Win32_Printer.PrinterState (obsolete property)
        match state {
            0 => PrinterState::Idle,              // Idle
            1 => PrinterState::Paused,            // Paused
            2 => PrinterState::Error,             // Error
            3 => PrinterState::PendingDeletion,   // Pending Deletion
            4 => PrinterState::PaperJam,          // Paper Jam
            5 => PrinterState::PaperOut,          // Paper Out
            6 => PrinterState::ManualFeed,        // Manual Feed
            7 => PrinterState::PaperProblem,      // Paper Problem
            8 => PrinterState::Offline,           // Offline
            9 => PrinterState::IOActive,          // I/O Active
            10 => PrinterState::Busy,             // Busy
            11 => PrinterState::Printing,         // Printing
            12 => PrinterState::OutputBinFull,    // Output Bin Full
            13 => PrinterState::NotAvailable,     // Not Available
            14 => PrinterState::Waiting,          // Waiting
            15 => PrinterState::Processing,       // Processing
            16 => PrinterState::Initialization,   // Initialization
            17 => PrinterState::Warmup,           // Warming Up
            18 => PrinterState::TonerLow,         // Toner Low
            19 => PrinterState::NoToner,          // No Toner
            20 => PrinterState::PagePunt,         // Page Punt
            21 => PrinterState::UserInterventionRequired, // User Intervention Required
            22 => PrinterState::OutOfMemory,      // Out of Memory
            23 => PrinterState::DoorOpen,         // Door Open
            24 => PrinterState::ServerUnknown,    // Server Unknown
            25 => PrinterState::PowerSave,        // Power Save
            128 => PrinterState::Offline,         // Legacy offline state
            _ => PrinterState::StateUnknown,      // Unmapped values
        }
    }

    /// Returns a human-readable description of this printer state.
    ///
    /// # Returns
    /// A static string describing the state
    pub fn description(&self) -> &'static str {
        match self {
            PrinterState::Idle => "Idle",
            PrinterState::Paused => "Paused",
            PrinterState::Error => "Error",
            PrinterState::PendingDeletion => "Pending Deletion",
            PrinterState::PaperJam => "Paper Jam",
            PrinterState::PaperOut => "Paper Out",
            PrinterState::ManualFeed => "Manual Feed",
            PrinterState::PaperProblem => "Paper Problem",
            PrinterState::Offline => "Offline",
            PrinterState::IOActive => "I/O Active",
            PrinterState::Busy => "Busy",
            PrinterState::Printing => "Printing",
            PrinterState::OutputBinFull => "Output Bin Full",
            PrinterState::NotAvailable => "Not Available",
            PrinterState::Waiting => "Waiting",
            PrinterState::Processing => "Processing",
            PrinterState::Initialization => "Initialization",
            PrinterState::Warmup => "Warming Up",
            PrinterState::TonerLow => "Toner Low",
            PrinterState::NoToner => "No Toner",
            PrinterState::PagePunt => "Page Punt",
            PrinterState::UserInterventionRequired => "User Intervention Required",
            PrinterState::OutOfMemory => "Out of Memory",
            PrinterState::DoorOpen => "Door Open",
            PrinterState::ServerUnknown => "Server Unknown",
            PrinterState::PowerSave => "Power Save",
            PrinterState::StateUnknown => "State Unknown",
        }
    }

    /// Converts PrinterState to equivalent PrinterStatus when possible
    ///
    /// # Returns
    /// PrinterStatus equivalent or StatusUnknown if no mapping exists
    pub fn to_printer_status(&self) -> PrinterStatus {
        match self {
            PrinterState::Idle => PrinterStatus::Idle,
            PrinterState::Printing => PrinterStatus::Printing,
            PrinterState::Warmup => PrinterStatus::Warmup,
            PrinterState::Offline => PrinterStatus::Offline,
            _ => PrinterStatus::StatusUnknown,
        }
    }
}

impl std::fmt::Display for PrinterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl std::fmt::Display for PrinterState {
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
            Some(0) => ErrorState::NoError,         // Unknown (but often means no error in practice)
            Some(1) => ErrorState::Other,           // Other
            Some(2) => ErrorState::NoError,         // No Error
            Some(3) => ErrorState::LowPaper,        // Low Paper
            Some(4) => ErrorState::NoPaper,         // No Paper
            Some(5) => ErrorState::LowToner,        // Low Toner
            Some(6) => ErrorState::NoToner,         // No Toner
            Some(7) => ErrorState::DoorOpen,        // Door Open
            Some(8) => ErrorState::Jammed,          // Jammed
            Some(9) => ErrorState::Other,           // Offline (map to Other since we have separate offline status)
            Some(10) => ErrorState::ServiceRequested, // Service Requested
            Some(11) => ErrorState::OutputBinFull,  // Output Bin Full
            _ => ErrorState::UnknownError,          // Unmapped values
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
    #[serde(rename = "ExtendedPrinterStatus")]
    pub extended_printer_status: Option<u32>,
    #[serde(rename = "ExtendedDetectedErrorState")]
    pub extended_detected_error_state: Option<u32>,
    #[serde(rename = "Status")]
    pub status: Option<String>,
}

/// Represents a printer and its current state
#[derive(Debug, Clone)]
pub struct Printer {
    name: String,
    status: PrinterStatus,
    state: Option<PrinterState>,
    error_state: ErrorState,
    is_offline: bool,
    is_default: bool,
    
    // Raw WMI status codes for detailed analysis
    printer_status_code: Option<u32>,          // PrinterStatus (1-7)
    printer_state_code: Option<u32>,           // PrinterState (0-25, obsolete)
    detected_error_state_code: Option<u32>,    // DetectedErrorState (0-11)
    extended_detected_error_state_code: Option<u32>, // ExtendedDetectedErrorState
    extended_printer_status_code: Option<u32>, // ExtendedPrinterStatus
    wmi_status: Option<String>,                // Status property (OK, Degraded, etc.)
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
            state: None,
            error_state,
            is_offline,
            is_default,
            printer_status_code: None,
            printer_state_code: None,
            detected_error_state_code: None,
            extended_detected_error_state_code: None,
            extended_printer_status_code: None,
            wmi_status: None,
        }
    }

    /// Creates a new Printer instance with both status and state information.
    pub fn new_with_state(
        name: String,
        status: PrinterStatus,
        state: Option<PrinterState>,
        error_state: ErrorState,
        is_offline: bool,
        is_default: bool,
    ) -> Self {
        Self {
            name,
            status,
            state,
            error_state,
            is_offline,
            is_default,
            printer_status_code: None,
            printer_state_code: None,
            detected_error_state_code: None,
            extended_detected_error_state_code: None,
            extended_printer_status_code: None,
            wmi_status: None,
        }
    }

    /// Creates a new Printer instance with complete WMI information.
    #[cfg(windows)]
    pub fn new_with_wmi(
        name: String,
        status: PrinterStatus,
        state: Option<PrinterState>,
        error_state: ErrorState,
        is_offline: bool,
        is_default: bool,
        printer_status_code: Option<u32>,
        printer_state_code: Option<u32>,
        detected_error_state_code: Option<u32>,
        extended_detected_error_state_code: Option<u32>,
        extended_printer_status_code: Option<u32>,
        wmi_status: Option<String>,
    ) -> Self {
        Self {
            name,
            status,
            state,
            error_state,
            is_offline,
            is_default,
            printer_status_code,
            printer_state_code,
            detected_error_state_code,
            extended_detected_error_state_code,
            extended_printer_status_code,
            wmi_status,
        }
    }

    /// Returns the printer's name as registered in the system.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the printer's current operational status.
    pub fn status(&self) -> &PrinterStatus {
        &self.status
    }

    /// Returns a reference to the printer's current state (if available from obsolete property).
    pub fn state(&self) -> Option<&PrinterState> {
        self.state.as_ref()
    }

    /// Returns a human-readable description of the printer's current status.
    pub fn status_description(&self) -> &'static str {
        self.status.description()
    }

    /// Returns a reference to the printer's current error state.
    pub fn error_state(&self) -> &ErrorState {
        &self.error_state
    }

    /// Returns a human-readable description of the printer's current error state.
    pub fn error_description(&self) -> &'static str {
        self.error_state.description()
    }

    /// Checks whether the printer is currently offline or disconnected.
    pub fn is_offline(&self) -> bool {
        self.is_offline
    }

    /// Checks whether this printer is set as the system's default printer.
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Checks whether the printer currently has any error conditions.
    pub fn has_error(&self) -> bool {
        self.error_state.is_error()
    }

    // Raw WMI Status Code Getters
    
    /// Returns the raw PrinterStatus code (1-7, current/recommended property)
    pub fn printer_status_code(&self) -> Option<u32> {
        self.printer_status_code
    }
    
    /// Returns the raw PrinterState code (0-25, obsolete property)
    pub fn printer_state_code(&self) -> Option<u32> {
        self.printer_state_code
    }
    
    /// Returns the raw DetectedErrorState code (0-11)
    pub fn detected_error_state_code(&self) -> Option<u32> {
        self.detected_error_state_code
    }
    
    /// Returns the raw ExtendedDetectedErrorState code
    pub fn extended_detected_error_state_code(&self) -> Option<u32> {
        self.extended_detected_error_state_code
    }
    
    /// Returns the raw ExtendedPrinterStatus code
    pub fn extended_printer_status_code(&self) -> Option<u32> {
        self.extended_printer_status_code
    }
    
    /// Returns the WMI Status property string (OK, Degraded, Error, etc.)
    pub fn wmi_status(&self) -> Option<&str> {
        self.wmi_status.as_deref()
    }

    // WMI Status Description Getters
    
    /// Returns human-readable description of PrinterStatus code
    pub fn printer_status_description(&self) -> Option<&'static str> {
        self.printer_status_code.map(|code| match code {
            1 => "Other",
            2 => "Unknown", 
            3 => "Idle",
            4 => "Printing",
            5 => "Warmup",
            6 => "Stopped Printing",
            7 => "Offline",
            _ => "Unknown Status Code",
        })
    }
    
    /// Returns human-readable description of PrinterState code (obsolete property)
    pub fn printer_state_description(&self) -> Option<&'static str> {
        self.printer_state_code.map(|code| match code {
            0 => "Idle",
            1 => "Paused", 
            2 => "Error",
            3 => "Pending Deletion",
            4 => "Paper Jam",
            5 => "Paper Out",
            6 => "Manual Feed",
            7 => "Paper Problem",
            8 => "Offline",
            9 => "I/O Active",
            10 => "Busy",
            11 => "Printing",
            12 => "Output Bin Full",
            13 => "Not Available",
            14 => "Waiting",
            15 => "Processing",
            16 => "Initialization",
            17 => "Warming Up",
            18 => "Toner Low",
            19 => "No Toner",
            20 => "Page Punt",
            21 => "User Intervention Required",
            22 => "Out of Memory",
            23 => "Door Open",
            24 => "Server Unknown",
            25 => "Power Save",
            128 => "Offline (Legacy)",
            _ => "Unknown State Code",
        })
    }
    
    /// Returns human-readable description of DetectedErrorState code
    pub fn detected_error_state_description(&self) -> Option<&'static str> {
        self.detected_error_state_code.map(|code| match code {
            0 => "Unknown (often No Error in practice)",
            1 => "Other",
            2 => "No Error",
            3 => "Low Paper",
            4 => "No Paper",
            5 => "Low Toner",
            6 => "No Toner",
            7 => "Door Open",
            8 => "Jammed",
            9 => "Offline",
            10 => "Service Requested",
            11 => "Output Bin Full",
            _ => "Unknown Error Code",
        })
    }
    
    /// Returns human-readable description of ExtendedPrinterStatus code
    pub fn extended_printer_status_description(&self) -> Option<&'static str> {
        self.extended_printer_status_code.map(|code| match code {
            1 => "Other",
            2 => "Unknown",
            3 => "Idle",
            4 => "Printing",
            5 => "Warmup",
            6 => "Stopped Printing", 
            7 => "Offline",
            8 => "Paused",
            9 => "Error",
            10 => "Busy",
            11 => "Not Available",
            12 => "Waiting",
            13 => "Processing",
            14 => "Initialization",
            15 => "Power Save",
            _ => "Unknown Extended Status Code",
        })
    }
}

#[cfg(windows)]
impl From<Win32Printer> for Printer {
    /// Converts a WMI Win32_Printer object into a Printer instance.
    ///
    /// This implementation prioritizes PrinterStatus (current) over PrinterState (obsolete)
    /// according to Microsoft recommendations.
    fn from(wmi_printer: Win32Printer) -> Self {
        // First, try to get status from PrinterStatus (current/recommended property)
        let status = PrinterStatus::from_u32(wmi_printer.printer_status);
        
        // Also get PrinterState (obsolete property) for additional detail if needed
        let state = wmi_printer.printer_state.map(PrinterState::from_u32);
        
        // If PrinterStatus is unknown but we have PrinterState, try to convert
        let final_status = match (&status, &state) {
            (PrinterStatus::StatusUnknown, Some(ps)) => ps.to_printer_status(),
            _ => status,
        };

        // Determine offline status using multiple WMI properties for comprehensive detection
        let is_offline = wmi_printer.work_offline.unwrap_or(false) 
            || matches!(final_status, PrinterStatus::Offline)
            || state.as_ref().map_or(false, |s| matches!(s, 
                PrinterState::Offline | 
                PrinterState::Error | 
                PrinterState::NotAvailable | 
                PrinterState::ServerUnknown
            ))
            // ExtendedPrinterStatus 7 = Offline
            || wmi_printer.extended_printer_status == Some(7)
            // Status property indicating problematic states
            || wmi_printer.status.as_ref().map_or(false, |s| matches!(s.as_str(), 
                "Degraded" | "Error" | "No Contact" | "Lost Comm" | "NonRecover"
            ));

        Self {
            name: wmi_printer
                .name
                .unwrap_or_else(|| "Unknown Printer".to_string()),
            status: final_status,
            state,
            error_state: ErrorState::from_u32(wmi_printer.detected_error_state),
            is_offline,
            is_default: wmi_printer.default.unwrap_or(false),
            
            // Store all raw WMI status codes for detailed analysis
            printer_status_code: wmi_printer.printer_status,
            printer_state_code: wmi_printer.printer_state,
            detected_error_state_code: wmi_printer.detected_error_state,
            extended_detected_error_state_code: wmi_printer.extended_detected_error_state,
            extended_printer_status_code: wmi_printer.extended_printer_status,
            wmi_status: wmi_printer.status,
        }
    }
}

impl PartialEq for Printer {
    /// Compares two Printer instances for equality.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.status == other.status
            && self.state == other.state
            && self.error_state == other.error_state
            && self.is_offline == other.is_offline
            && self.printer_status_code == other.printer_status_code
            && self.printer_state_code == other.printer_state_code
            && self.detected_error_state_code == other.detected_error_state_code
            && self.extended_detected_error_state_code == other.extended_detected_error_state_code
            && self.extended_printer_status_code == other.extended_printer_status_code
            && self.wmi_status == other.wmi_status
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
    fn test_printer_state_display() {
        assert_eq!(PrinterState::PaperJam.to_string(), "Paper Jam");
        assert_eq!(PrinterState::TonerLow.to_string(), "Toner Low");
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

    #[test]
    fn test_printer_state_to_status_conversion() {
        assert_eq!(PrinterState::Idle.to_printer_status(), PrinterStatus::Idle);
        assert_eq!(PrinterState::Printing.to_printer_status(), PrinterStatus::Printing);
        assert_eq!(PrinterState::PaperJam.to_printer_status(), PrinterStatus::StatusUnknown);
    }
}