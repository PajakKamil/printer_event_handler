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

/// Represents a printer's state using .NET PrintQueueStatus flags
///
/// This enum represents the actual WMI PrinterState values which correspond to
/// the .NET System.Printing.PrintQueueStatus enumeration flags.
/// See: <https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus>
#[derive(Debug, Clone, PartialEq)]
pub enum PrinterState {
    None,                     // 0 - No status
    Paused,                   // 1 - The print queue is paused
    Error,                    // 2 - The print queue has an error
    PendingDeletion,          // 4 - The print queue is being deleted
    PaperJam,                 // 8 - The printer has a paper jam
    PaperOut,                 // 16 - The printer is out of paper
    ManualFeed,               // 32 - The printer needs manual paper feed
    PaperProblem,             // 64 - The printer has a paper problem
    Offline,                  // 128 - The printer is offline
    IOActive,                 // 256 - The printer's input/output is active
    Busy,                     // 512 - The printer is busy
    Printing,                 // 1024 - The printer is printing
    OutputBinFull,            // 2048 - The printer's output bin is full
    NotAvailable,             // 4096 - The printer is not available
    Waiting,                  // 8192 - The printer is waiting
    Processing,               // 16384 - The printer is processing a job
    Initializing,             // 32768 - The printer is initializing
    WarmingUp,                // 65536 - The printer is warming up
    TonerLow,                 // 131072 - The printer is low on toner
    NoToner,                  // 262144 - The printer has no toner
    PagePunt,                 // 524288 - The printer cannot print the current page
    UserInterventionRequired, // 1048576 - The printer needs user intervention
    OutOfMemory,              // 2097152 - The printer is out of memory
    DoorOpen,                 // 4194304 - The printer door is open
    ServerUnknown,            // 8388608 - The print server is unknown
    PowerSave,                // 16777216 - The printer is in power save mode
    StatusUnknown,            // Fallback for unmapped values
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
    /// Creates a PrinterState from a WMI PrinterState value.
    ///
    /// # Arguments
    /// * `state` - WMI Win32_Printer.PrinterState value (actually .NET PrintQueueStatus flags)
    ///
    /// # Returns
    /// Corresponding PrinterState enum variant for the most significant flag
    #[cfg(windows)]
    pub(crate) fn from_u32(state: u32) -> Self {
        // Handle .NET PrintQueueStatus flag values - return the most significant flag
        // Priority order: Error conditions first, then active states, then idle states

        if state == 0 {
            return PrinterState::None;
        }

        // Error and problem states (highest priority)
        if state & 4194304 != 0 {
            // DoorOpen
            PrinterState::DoorOpen
        } else if state & 2 != 0 {
            // Error
            PrinterState::Error
        } else if state & 8 != 0 {
            // PaperJam
            PrinterState::PaperJam
        } else if state & 16 != 0 {
            // PaperOut
            PrinterState::PaperOut
        } else if state & 64 != 0 {
            // PaperProblem
            PrinterState::PaperProblem
        } else if state & 131072 != 0 {
            // TonerLow
            PrinterState::TonerLow
        } else if state & 262144 != 0 {
            // NoToner
            PrinterState::NoToner
        } else if state & 2097152 != 0 {
            // OutOfMemory
            PrinterState::OutOfMemory
        } else if state & 1048576 != 0 {
            // UserInterventionRequired
            PrinterState::UserInterventionRequired
        } else if state & 524288 != 0 {
            // PagePunt
            PrinterState::PagePunt
        } else if state & 128 != 0 {
            // Offline
            PrinterState::Offline
        } else if state & 4096 != 0 {
            // NotAvailable
            PrinterState::NotAvailable
        } else if state & 8388608 != 0 {
            // ServerUnknown
            PrinterState::ServerUnknown

        // Active processing states
        } else if state & 1024 != 0 {
            // Printing
            PrinterState::Printing
        } else if state & 16384 != 0 {
            // Processing
            PrinterState::Processing
        } else if state & 32768 != 0 {
            // Initializing
            PrinterState::Initializing
        } else if state & 65536 != 0 {
            // WarmingUp
            PrinterState::WarmingUp
        } else if state & 512 != 0 {
            // Busy
            PrinterState::Busy
        } else if state & 256 != 0 {
            // IOActive
            PrinterState::IOActive

        // Waiting and paused states
        } else if state & 1 != 0 {
            // Paused
            PrinterState::Paused
        } else if state & 8192 != 0 {
            // Waiting
            PrinterState::Waiting
        } else if state & 32 != 0 {
            // ManualFeed
            PrinterState::ManualFeed
        } else if state & 2048 != 0 {
            // OutputBinFull
            PrinterState::OutputBinFull

        // Maintenance and special states
        } else if state & 16777216 != 0 {
            // PowerSave
            PrinterState::PowerSave
        } else if state & 4 != 0 {
            // PendingDeletion
            PrinterState::PendingDeletion
        } else {
            PrinterState::StatusUnknown
        }
    }

    /// Returns a human-readable description of this printer state.
    ///
    /// # Returns
    /// A static string describing the status
    pub fn description(&self) -> &'static str {
        match self {
            PrinterState::None => "None",
            PrinterState::Paused => "Paused",
            PrinterState::Error => "Error",
            PrinterState::PendingDeletion => "Pending Deletion",
            PrinterState::PaperJam => "Paper Jam",
            PrinterState::PaperOut => "Paper Out",
            PrinterState::ManualFeed => "Manual Feed Required",
            PrinterState::PaperProblem => "Paper Problem",
            PrinterState::Offline => "Offline",
            PrinterState::IOActive => "I/O Active",
            PrinterState::Busy => "Busy",
            PrinterState::Printing => "Printing",
            PrinterState::OutputBinFull => "Output Bin Full",
            PrinterState::NotAvailable => "Not Available",
            PrinterState::Waiting => "Waiting",
            PrinterState::Processing => "Processing Job",
            PrinterState::Initializing => "Initializing",
            PrinterState::WarmingUp => "Warming Up",
            PrinterState::TonerLow => "Toner Low",
            PrinterState::NoToner => "No Toner",
            PrinterState::PagePunt => "Page Punt",
            PrinterState::UserInterventionRequired => "User Intervention Required",
            PrinterState::OutOfMemory => "Out of Memory",
            PrinterState::DoorOpen => "Door Open",
            PrinterState::ServerUnknown => "Print Server Unknown",
            PrinterState::PowerSave => "Power Save Mode",
            PrinterState::StatusUnknown => "Status Unknown",
        }
    }

    /// Converts PrinterState to equivalent PrinterStatus when possible
    ///
    /// # Returns
    /// PrinterStatus equivalent or StatusUnknown if no mapping exists
    pub fn to_printer_status(&self) -> PrinterStatus {
        match self {
            PrinterState::None => PrinterStatus::Idle,
            PrinterState::Printing => PrinterStatus::Printing,
            PrinterState::WarmingUp => PrinterStatus::Warmup,
            PrinterState::Offline => PrinterStatus::Offline,
            PrinterState::Paused => PrinterStatus::StoppedPrinting,
            PrinterState::Error
            | PrinterState::PaperJam
            | PrinterState::PaperOut
            | PrinterState::DoorOpen => PrinterStatus::Other, // Error conditions
            _ => PrinterStatus::StatusUnknown,
        }
    }

    /// Checks if this status represents an error condition
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            PrinterState::Error
                | PrinterState::PaperJam
                | PrinterState::PaperOut
                | PrinterState::PaperProblem
                | PrinterState::DoorOpen
                | PrinterState::OutOfMemory
                | PrinterState::NoToner
                | PrinterState::UserInterventionRequired
        )
    }

    /// Checks if this status represents an offline condition
    pub fn is_offline(&self) -> bool {
        matches!(
            self,
            PrinterState::Offline | PrinterState::NotAvailable | PrinterState::ServerUnknown
        )
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Represents a change in a specific printer property
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyChange {
    Name {
        old: String,
        new: String,
    },
    Status {
        old: PrinterStatus,
        new: PrinterStatus,
    },
    State {
        old: Option<PrinterState>,
        new: Option<PrinterState>,
    },
    ErrorState {
        old: ErrorState,
        new: ErrorState,
    },
    IsOffline {
        old: bool,
        new: bool,
    },
    IsDefault {
        old: bool,
        new: bool,
    },
    PrinterStatusCode {
        old: Option<u32>,
        new: Option<u32>,
    },
    PrinterStateCode {
        old: Option<u32>,
        new: Option<u32>,
    },
    DetectedErrorStateCode {
        old: Option<u32>,
        new: Option<u32>,
    },
    ExtendedDetectedErrorStateCode {
        old: Option<u32>,
        new: Option<u32>,
    },
    ExtendedPrinterStatusCode {
        old: Option<u32>,
        new: Option<u32>,
    },
    WmiStatus {
        old: Option<String>,
        new: Option<String>,
    },
}

impl PropertyChange {
    /// Returns the name of the property that changed
    pub fn property_name(&self) -> &'static str {
        match self {
            PropertyChange::Name { .. } => "Name",
            PropertyChange::Status { .. } => "Status",
            PropertyChange::State { .. } => "State",
            PropertyChange::ErrorState { .. } => "ErrorState",
            PropertyChange::IsOffline { .. } => "IsOffline",
            PropertyChange::IsDefault { .. } => "IsDefault",
            PropertyChange::PrinterStatusCode { .. } => "PrinterStatusCode",
            PropertyChange::PrinterStateCode { .. } => "PrinterStateCode",
            PropertyChange::DetectedErrorStateCode { .. } => "DetectedErrorStateCode",
            PropertyChange::ExtendedDetectedErrorStateCode { .. } => {
                "ExtendedDetectedErrorStateCode"
            }
            PropertyChange::ExtendedPrinterStatusCode { .. } => "ExtendedPrinterStatusCode",
            PropertyChange::WmiStatus { .. } => "WmiStatus",
        }
    }

    /// Returns a human-readable description of the change
    pub fn description(&self) -> String {
        match self {
            PropertyChange::Name { old, new } => format!("Name: '{}' → '{}'", old, new),
            PropertyChange::Status { old, new } => {
                format!("Status: {} → {}", old.description(), new.description())
            }
            PropertyChange::State { old, new } => {
                let old_desc = old.as_ref().map(|s| s.description()).unwrap_or("None");
                let new_desc = new.as_ref().map(|s| s.description()).unwrap_or("None");
                format!("State: {} → {}", old_desc, new_desc)
            }
            PropertyChange::ErrorState { old, new } => {
                format!("ErrorState: {} → {}", old.description(), new.description())
            }
            PropertyChange::IsOffline { old, new } => format!("IsOffline: {} → {}", old, new),
            PropertyChange::IsDefault { old, new } => format!("IsDefault: {} → {}", old, new),
            PropertyChange::PrinterStatusCode { old, new } => {
                format!("PrinterStatusCode: {:?} → {:?}", old, new)
            }
            PropertyChange::PrinterStateCode { old, new } => {
                format!("PrinterStateCode: {:?} → {:?}", old, new)
            }
            PropertyChange::DetectedErrorStateCode { old, new } => {
                format!("DetectedErrorStateCode: {:?} → {:?}", old, new)
            }
            PropertyChange::ExtendedDetectedErrorStateCode { old, new } => {
                format!("ExtendedDetectedErrorStateCode: {:?} → {:?}", old, new)
            }
            PropertyChange::ExtendedPrinterStatusCode { old, new } => {
                format!("ExtendedPrinterStatusCode: {:?} → {:?}", old, new)
            }
            PropertyChange::WmiStatus { old, new } => format!("WmiStatus: {:?} → {:?}", old, new),
        }
    }
}

/// Contains all property changes detected between two printer states
#[derive(Debug, Clone)]
pub struct PrinterChanges {
    /// The printer name these changes apply to
    pub printer_name: String,
    /// List of individual property changes
    pub changes: Vec<PropertyChange>,
    /// Timestamp when the changes were detected
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PrinterChanges {
    /// Creates a new empty PrinterChanges instance
    pub fn new(printer_name: String) -> Self {
        Self {
            printer_name,
            changes: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Checks if any changes were detected
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Returns the number of properties that changed
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Checks if a specific property changed
    pub fn has_property_change(&self, property_name: &str) -> bool {
        self.changes
            .iter()
            .any(|change| change.property_name() == property_name)
    }

    /// Gets all changes for a specific property
    pub fn get_property_changes(&self, property_name: &str) -> Vec<&PropertyChange> {
        self.changes
            .iter()
            .filter(|change| change.property_name() == property_name)
            .collect()
    }

    /// Returns a summary string of all changes
    pub fn summary(&self) -> String {
        if self.changes.is_empty() {
            return "No changes detected".to_string();
        }

        format!(
            "{} properties changed: {}",
            self.changes.len(),
            self.changes
                .iter()
                .map(|c| c.property_name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// WMI status codes for creating Printer instances
#[cfg(windows)]
#[derive(Debug)]
pub struct WmiStatusCodes {
    pub printer_status_code: Option<u32>,
    pub printer_state_code: Option<u32>,
    pub detected_error_state_code: Option<u32>,
    pub extended_detected_error_state_code: Option<u32>,
    pub extended_printer_status_code: Option<u32>,
    pub wmi_status: Option<String>,
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
    printer_status_code: Option<u32>,       // PrinterStatus (1-7)
    printer_state_code: Option<u32>,        // PrinterState (0-25, obsolete)
    detected_error_state_code: Option<u32>, // DetectedErrorState (0-11)
    extended_detected_error_state_code: Option<u32>, // ExtendedDetectedErrorState
    extended_printer_status_code: Option<u32>, // ExtendedPrinterStatus
    wmi_status: Option<String>,             // Status property (OK, Degraded, etc.)
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
        wmi_codes: WmiStatusCodes,
    ) -> Self {
        Self {
            name,
            status,
            state,
            error_state,
            is_offline,
            is_default,
            printer_status_code: wmi_codes.printer_status_code,
            printer_state_code: wmi_codes.printer_state_code,
            detected_error_state_code: wmi_codes.detected_error_state_code,
            extended_detected_error_state_code: wmi_codes.extended_detected_error_state_code,
            extended_printer_status_code: wmi_codes.extended_printer_status_code,
            wmi_status: wmi_codes.wmi_status,
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
            // Documented Win32_Printer.PrinterState values (0-25)
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

            // Real-world bitwise flag values
            1024 => "Printing (Flag)",
            16384 => "Initialization (Flag)",
            2048 => "Processing (Flag)",
            4096 => "Busy (Flag)",
            8192 => "Warming Up (Flag)",
            32768 => "Paper Out (Flag)",
            65536 => "Error (Flag)",

            // For unknown values, try to interpret flags
            _ => {
                if code & 1024 != 0 {
                    "Printing (Multi-flag)"
                } else if code & 16384 != 0 {
                    "Initialization (Multi-flag)"
                } else if code & 2048 != 0 {
                    "Processing (Multi-flag)"
                } else if code & 4096 != 0 {
                    "Busy (Multi-flag)"
                } else if code & 8192 != 0 {
                    "Warming Up (Multi-flag)"
                } else if code & 32768 != 0 {
                    "Paper Out (Multi-flag)"
                } else if code & 65536 != 0 {
                    "Error (Multi-flag)"
                } else if code & 1 != 0 {
                    "Paused (Multi-flag)"
                } else {
                    "Unknown State Code"
                }
            }
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

    /// Compares this printer with another and returns detailed changes
    pub fn compare_with(&self, other: &Printer) -> PrinterChanges {
        let mut changes = PrinterChanges::new(self.name.clone());

        // Check each property for changes
        if self.name != other.name {
            changes.changes.push(PropertyChange::Name {
                old: self.name.clone(),
                new: other.name.clone(),
            });
        }

        if self.status != other.status {
            changes.changes.push(PropertyChange::Status {
                old: self.status.clone(),
                new: other.status.clone(),
            });
        }

        if self.state != other.state {
            changes.changes.push(PropertyChange::State {
                old: self.state.clone(),
                new: other.state.clone(),
            });
        }

        if self.error_state != other.error_state {
            changes.changes.push(PropertyChange::ErrorState {
                old: self.error_state.clone(),
                new: other.error_state.clone(),
            });
        }

        if self.is_offline != other.is_offline {
            changes.changes.push(PropertyChange::IsOffline {
                old: self.is_offline,
                new: other.is_offline,
            });
        }

        if self.is_default != other.is_default {
            changes.changes.push(PropertyChange::IsDefault {
                old: self.is_default,
                new: other.is_default,
            });
        }

        if self.printer_status_code != other.printer_status_code {
            changes.changes.push(PropertyChange::PrinterStatusCode {
                old: self.printer_status_code,
                new: other.printer_status_code,
            });
        }

        if self.printer_state_code != other.printer_state_code {
            changes.changes.push(PropertyChange::PrinterStateCode {
                old: self.printer_state_code,
                new: other.printer_state_code,
            });
        }

        if self.detected_error_state_code != other.detected_error_state_code {
            changes
                .changes
                .push(PropertyChange::DetectedErrorStateCode {
                    old: self.detected_error_state_code,
                    new: other.detected_error_state_code,
                });
        }

        if self.extended_detected_error_state_code != other.extended_detected_error_state_code {
            changes
                .changes
                .push(PropertyChange::ExtendedDetectedErrorStateCode {
                    old: self.extended_detected_error_state_code,
                    new: other.extended_detected_error_state_code,
                });
        }

        if self.extended_printer_status_code != other.extended_printer_status_code {
            changes
                .changes
                .push(PropertyChange::ExtendedPrinterStatusCode {
                    old: self.extended_printer_status_code,
                    new: other.extended_printer_status_code,
                });
        }

        if self.wmi_status != other.wmi_status {
            changes.changes.push(PropertyChange::WmiStatus {
                old: self.wmi_status.clone(),
                new: other.wmi_status.clone(),
            });
        }

        changes
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
            || state.as_ref().is_some_and(|s| matches!(s,
                PrinterState::Offline |
                PrinterState::Error |
                PrinterState::NotAvailable |
                PrinterState::ServerUnknown
            ))
            // ExtendedPrinterStatus 7 = Offline
            || wmi_printer.extended_printer_status == Some(7)
            // Status property indicating problematic states
            || wmi_printer.status.as_ref().is_some_and(|s| matches!(s.as_str(),
                "Degraded" | "Error" | "No Contact" | "Lost Comm" | "NonRecover"
            ));

        let wmi_codes = WmiStatusCodes {
            printer_status_code: wmi_printer.printer_status,
            printer_state_code: wmi_printer.printer_state,
            detected_error_state_code: wmi_printer.detected_error_state,
            extended_detected_error_state_code: wmi_printer.extended_detected_error_state,
            extended_printer_status_code: wmi_printer.extended_printer_status,
            wmi_status: wmi_printer.status,
        };

        Self::new_with_wmi(
            wmi_printer
                .name
                .unwrap_or_else(|| "Unknown Printer".to_string()),
            final_status,
            state,
            ErrorState::from_u32(wmi_printer.detected_error_state),
            is_offline,
            wmi_printer.default.unwrap_or(false),
            wmi_codes,
        )
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
        assert_eq!(PrinterState::None.to_printer_status(), PrinterStatus::Idle);
        assert_eq!(
            PrinterState::Printing.to_printer_status(),
            PrinterStatus::Printing
        );
        assert_eq!(
            PrinterState::PaperJam.to_printer_status(),
            PrinterStatus::Other
        );
    }
}
