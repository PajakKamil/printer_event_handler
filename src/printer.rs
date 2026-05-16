#[cfg(windows)]
use serde::Deserialize;

// .NET PrintQueueStatus flag values used by Win32_Printer.PrinterState.
// Defined once here so the priority chain in `PrinterState::from_u32` and the
// description tables in `Printer::printer_state_description` stay in sync.
// Reference: https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus
const PRINTER_STATE_PAUSED: u32 = 1;
const PRINTER_STATE_ERROR: u32 = 2;
const PRINTER_STATE_PENDING_DELETION: u32 = 4;
const PRINTER_STATE_PAPER_JAM: u32 = 8;
const PRINTER_STATE_PAPER_OUT: u32 = 16;
const PRINTER_STATE_MANUAL_FEED: u32 = 32;
const PRINTER_STATE_PAPER_PROBLEM: u32 = 64;
const PRINTER_STATE_OFFLINE: u32 = 128;
const PRINTER_STATE_IO_ACTIVE: u32 = 256;
const PRINTER_STATE_BUSY: u32 = 512;
const PRINTER_STATE_PRINTING: u32 = 1024;
const PRINTER_STATE_OUTPUT_BIN_FULL: u32 = 2048;
const PRINTER_STATE_NOT_AVAILABLE: u32 = 4096;
const PRINTER_STATE_WAITING: u32 = 8192;
const PRINTER_STATE_PROCESSING: u32 = 16_384;
const PRINTER_STATE_INITIALIZING: u32 = 32_768;
const PRINTER_STATE_WARMING_UP: u32 = 65_536;
const PRINTER_STATE_TONER_LOW: u32 = 131_072;
const PRINTER_STATE_NO_TONER: u32 = 262_144;
const PRINTER_STATE_PAGE_PUNT: u32 = 524_288;
const PRINTER_STATE_USER_INTERVENTION_REQUIRED: u32 = 1_048_576;
const PRINTER_STATE_OUT_OF_MEMORY: u32 = 2_097_152;
const PRINTER_STATE_DOOR_OPEN: u32 = 4_194_304;
const PRINTER_STATE_SERVER_UNKNOWN: u32 = 8_388_608;
const PRINTER_STATE_POWER_SAVE: u32 = 16_777_216;

/// Represents a printer's status (Win32_Printer.PrinterStatus - Current/Recommended)
///
/// This is the current WMI property for printer status information.
/// Values 1-7 according to Microsoft documentation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    /// Corresponding PrinterState enum variant for the most informative flag.
    ///
    /// Specific-cause bits (e.g. PaperJam, NoToner, DoorOpen) are checked before
    /// the generic `Error` flag because WMI typically OR's both together; returning
    /// the specific cause preserves information for the caller.
    #[cfg(windows)]
    pub(crate) fn from_u32(state: u32) -> Self {
        if state == 0 {
            return PrinterState::None;
        }

        // Specific error conditions first - these typically come OR'd with the
        // generic `Error` bit, so checking `Error` early would mask them.
        if state & PRINTER_STATE_DOOR_OPEN != 0 {
            PrinterState::DoorOpen
        } else if state & PRINTER_STATE_PAPER_JAM != 0 {
            PrinterState::PaperJam
        } else if state & PRINTER_STATE_PAPER_OUT != 0 {
            PrinterState::PaperOut
        } else if state & PRINTER_STATE_PAPER_PROBLEM != 0 {
            PrinterState::PaperProblem
        } else if state & PRINTER_STATE_NO_TONER != 0 {
            PrinterState::NoToner
        } else if state & PRINTER_STATE_TONER_LOW != 0 {
            PrinterState::TonerLow
        } else if state & PRINTER_STATE_OUT_OF_MEMORY != 0 {
            PrinterState::OutOfMemory
        } else if state & PRINTER_STATE_USER_INTERVENTION_REQUIRED != 0 {
            PrinterState::UserInterventionRequired
        } else if state & PRINTER_STATE_PAGE_PUNT != 0 {
            PrinterState::PagePunt

        // Reachability problems
        } else if state & PRINTER_STATE_OFFLINE != 0 {
            PrinterState::Offline
        } else if state & PRINTER_STATE_NOT_AVAILABLE != 0 {
            PrinterState::NotAvailable
        } else if state & PRINTER_STATE_SERVER_UNKNOWN != 0 {
            PrinterState::ServerUnknown

        // Generic error fallback - reached only when no specific cause is set.
        } else if state & PRINTER_STATE_ERROR != 0 {
            PrinterState::Error

        // Active processing states
        } else if state & PRINTER_STATE_PRINTING != 0 {
            PrinterState::Printing
        } else if state & PRINTER_STATE_PROCESSING != 0 {
            PrinterState::Processing
        } else if state & PRINTER_STATE_INITIALIZING != 0 {
            PrinterState::Initializing
        } else if state & PRINTER_STATE_WARMING_UP != 0 {
            PrinterState::WarmingUp
        } else if state & PRINTER_STATE_BUSY != 0 {
            PrinterState::Busy
        } else if state & PRINTER_STATE_IO_ACTIVE != 0 {
            PrinterState::IOActive

        // Waiting and paused states
        } else if state & PRINTER_STATE_PAUSED != 0 {
            PrinterState::Paused
        } else if state & PRINTER_STATE_WAITING != 0 {
            PrinterState::Waiting
        } else if state & PRINTER_STATE_MANUAL_FEED != 0 {
            PrinterState::ManualFeed
        } else if state & PRINTER_STATE_OUTPUT_BIN_FULL != 0 {
            PrinterState::OutputBinFull

        // Maintenance and special states
        } else if state & PRINTER_STATE_POWER_SAVE != 0 {
            PrinterState::PowerSave
        } else if state & PRINTER_STATE_PENDING_DELETION != 0 {
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

    /// Converts PrinterState to equivalent PrinterStatus when possible.
    ///
    /// PrinterStatus (values 1-7) only covers operational states - it has no
    /// dedicated error variants, so error-class PrinterState values are mapped
    /// to `StoppedPrinting` (printer halted due to a problem) rather than the
    /// generic `Other`. Callers should inspect [`Printer::error_state`] for the
    /// specific cause (PaperJam, NoToner, etc.).
    ///
    /// # Returns
    /// PrinterStatus equivalent or StatusUnknown if no meaningful mapping exists
    pub fn to_printer_status(&self) -> PrinterStatus {
        match self {
            PrinterState::None => PrinterStatus::Idle,
            PrinterState::Printing => PrinterStatus::Printing,
            PrinterState::WarmingUp => PrinterStatus::Warmup,

            // Unreachable / disconnected -> Offline
            PrinterState::Offline | PrinterState::NotAvailable | PrinterState::ServerUnknown => {
                PrinterStatus::Offline
            }

            // Stuck due to a problem the user must resolve - StoppedPrinting
            // matches the WMI semantics better than collapsing to `Other`,
            // which is itself a documented WMI value meaning "something else".
            PrinterState::Paused
            | PrinterState::Error
            | PrinterState::PaperJam
            | PrinterState::PaperOut
            | PrinterState::PaperProblem
            | PrinterState::DoorOpen
            | PrinterState::OutOfMemory
            | PrinterState::NoToner
            | PrinterState::UserInterventionRequired
            | PrinterState::PagePunt => PrinterStatus::StoppedPrinting,

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    /// Unions an explicit `is_offline` hint with what `status`/`state` already
    /// imply. The stored boolean cannot drift below the typed fields - if the
    /// status says Offline or the state is unreachable, the printer is offline
    /// regardless of the bool passed by the caller.
    fn derive_is_offline(status: &PrinterStatus, state: Option<&PrinterState>, hint: bool) -> bool {
        hint || matches!(status, PrinterStatus::Offline)
            || state.is_some_and(PrinterState::is_offline)
    }

    /// Creates a new Printer instance with the specified properties.
    ///
    /// # Arguments
    /// * `name` - The printer's name as it appears in the system
    /// * `status` - Current operational status of the printer
    /// * `error_state` - Current error condition, if any
    /// * `is_offline` - Offline hint. The stored value is `hint || status==Offline || state.is_offline()`,
    ///   so passing `false` here cannot silently override a typed Offline status.
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
        let is_offline = Self::derive_is_offline(&status, None, is_offline);
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
        let is_offline = Self::derive_is_offline(&status, state.as_ref(), is_offline);
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
        let is_offline = Self::derive_is_offline(&status, state.as_ref(), is_offline);
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
    ///
    /// Win32_Printer.PrinterState is documented by Microsoft as a uint32 with
    /// values 0-25 (the "obsolete" table). On modern Windows the same property
    /// is also observed returning .NET PrintQueueStatus bitwise flag values.
    /// This function recognises both: the documented 0-25 lookup wins for
    /// small values, and flag interpretation handles the rest.
    /// Reference: <https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printer>
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

            // .NET PrintQueueStatus single-flag values seen in the wild on
            // modern Windows. All of these are > 25 so there's no clash with
            // the documented table above.
            PRINTER_STATE_OFFLINE => "Offline (Flag)",
            PRINTER_STATE_PRINTING => "Printing (Flag)",
            PRINTER_STATE_OUTPUT_BIN_FULL => "Output Bin Full (Flag)",
            PRINTER_STATE_NOT_AVAILABLE => "Not Available (Flag)",
            PRINTER_STATE_WAITING => "Waiting (Flag)",
            PRINTER_STATE_PROCESSING => "Processing (Flag)",
            PRINTER_STATE_INITIALIZING => "Initialization (Flag)",
            PRINTER_STATE_WARMING_UP => "Warming Up (Flag)",
            PRINTER_STATE_TONER_LOW => "Toner Low (Flag)",
            PRINTER_STATE_NO_TONER => "No Toner (Flag)",
            PRINTER_STATE_PAGE_PUNT => "Page Punt (Flag)",
            PRINTER_STATE_USER_INTERVENTION_REQUIRED => "User Intervention Required (Flag)",
            PRINTER_STATE_OUT_OF_MEMORY => "Out of Memory (Flag)",
            PRINTER_STATE_DOOR_OPEN => "Door Open (Flag)",
            PRINTER_STATE_SERVER_UNKNOWN => "Server Unknown (Flag)",
            PRINTER_STATE_POWER_SAVE => "Power Save (Flag)",

            // Combined bitmask - report the highest-priority flag that's set,
            // mirroring `PrinterState::from_u32`.
            _ => {
                if code & PRINTER_STATE_DOOR_OPEN != 0 {
                    "Door Open (Multi-flag)"
                } else if code & PRINTER_STATE_NO_TONER != 0 {
                    "No Toner (Multi-flag)"
                } else if code & PRINTER_STATE_OUT_OF_MEMORY != 0 {
                    "Out of Memory (Multi-flag)"
                } else if code & PRINTER_STATE_OFFLINE != 0 {
                    "Offline (Multi-flag)"
                } else if code & PRINTER_STATE_ERROR != 0 {
                    "Error (Multi-flag)"
                } else if code & PRINTER_STATE_PRINTING != 0 {
                    "Printing (Multi-flag)"
                } else if code & PRINTER_STATE_PROCESSING != 0 {
                    "Processing (Multi-flag)"
                } else if code & PRINTER_STATE_INITIALIZING != 0 {
                    "Initializing (Multi-flag)"
                } else if code & PRINTER_STATE_WARMING_UP != 0 {
                    "Warming Up (Multi-flag)"
                } else if code & PRINTER_STATE_PAUSED != 0 {
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

    /// Returns human-readable description of ExtendedDetectedErrorState code.
    ///
    /// Microsoft documents this property's value set as identical to
    /// DetectedErrorState (0-11), so this mirrors
    /// [`Printer::detected_error_state_description`] but reads the raw
    /// ExtendedDetectedErrorState code.
    pub fn extended_detected_error_state_description(&self) -> Option<&'static str> {
        self.extended_detected_error_state_code
            .map(|code| match code {
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
                _ => "Unknown Extended Error Code",
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

        // Two arms so we only call `.clone()` on non-Copy fields - avoids the
        // `clippy::clone_on_copy` lint on bool/Option<u32> codes while keeping
        // the call site compact.
        macro_rules! diff {
            (clone $variant:ident, $field:ident) => {
                if self.$field != other.$field {
                    changes.changes.push(PropertyChange::$variant {
                        old: self.$field.clone(),
                        new: other.$field.clone(),
                    });
                }
            };
            (copy $variant:ident, $field:ident) => {
                if self.$field != other.$field {
                    changes.changes.push(PropertyChange::$variant {
                        old: self.$field,
                        new: other.$field,
                    });
                }
            };
        }

        diff!(clone Name, name);
        diff!(clone Status, status);
        diff!(clone State, state);
        diff!(clone ErrorState, error_state);
        diff!(copy IsOffline, is_offline);
        diff!(copy IsDefault, is_default);
        diff!(copy PrinterStatusCode, printer_status_code);
        diff!(copy PrinterStateCode, printer_state_code);
        diff!(copy DetectedErrorStateCode, detected_error_state_code);
        diff!(copy ExtendedDetectedErrorStateCode, extended_detected_error_state_code);
        diff!(copy ExtendedPrinterStatusCode, extended_printer_status_code);
        diff!(clone WmiStatus, wmi_status);

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
            && self.is_default == other.is_default
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

        // Error-class states now map to StoppedPrinting (not the generic `Other`
        // which is a real WMI value meaning "something else").
        assert_eq!(
            PrinterState::PaperJam.to_printer_status(),
            PrinterStatus::StoppedPrinting
        );
        assert_eq!(
            PrinterState::DoorOpen.to_printer_status(),
            PrinterStatus::StoppedPrinting
        );
        assert_eq!(
            PrinterState::NoToner.to_printer_status(),
            PrinterStatus::StoppedPrinting
        );
        assert_eq!(
            PrinterState::Paused.to_printer_status(),
            PrinterStatus::StoppedPrinting
        );

        // Reachability states all funnel to Offline.
        assert_eq!(
            PrinterState::Offline.to_printer_status(),
            PrinterStatus::Offline
        );
        assert_eq!(
            PrinterState::NotAvailable.to_printer_status(),
            PrinterStatus::Offline
        );
        assert_eq!(
            PrinterState::ServerUnknown.to_printer_status(),
            PrinterStatus::Offline
        );
    }

    #[test]
    fn test_printer_equality_with_is_default() {
        let printer1 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let printer2 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            false, // Different is_default
        );

        // Should not be equal because is_default differs
        assert_ne!(printer1, printer2);
    }

    #[test]
    fn test_printer_equality_complete() {
        let printer1 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let printer2 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        assert_eq!(printer1, printer2);
    }

    #[test]
    fn test_printer_state_is_error() {
        assert!(PrinterState::Error.is_error());
        assert!(PrinterState::PaperJam.is_error());
        assert!(PrinterState::PaperOut.is_error());
        assert!(PrinterState::DoorOpen.is_error());
        assert!(PrinterState::OutOfMemory.is_error());
        assert!(PrinterState::UserInterventionRequired.is_error());

        assert!(!PrinterState::None.is_error());
        assert!(!PrinterState::WarmingUp.is_error());
        assert!(!PrinterState::Printing.is_error());
    }

    #[test]
    fn test_printer_state_is_offline() {
        assert!(PrinterState::Offline.is_offline());
        assert!(PrinterState::NotAvailable.is_offline());
        assert!(PrinterState::ServerUnknown.is_offline());

        assert!(!PrinterState::Printing.is_offline());
        assert!(!PrinterState::None.is_offline());
    }

    #[test]
    fn test_property_change_name() {
        let change = PropertyChange::Name {
            old: "Old Name".to_string(),
            new: "New Name".to_string(),
        };

        assert_eq!(change.property_name(), "Name");
        assert!(change.description().contains("Old Name"));
        assert!(change.description().contains("New Name"));
    }

    #[test]
    fn test_property_change_status() {
        let change = PropertyChange::Status {
            old: PrinterStatus::Idle,
            new: PrinterStatus::Printing,
        };

        assert_eq!(change.property_name(), "Status");
        assert!(change.description().contains("Idle"));
        assert!(change.description().contains("Printing"));
    }

    #[test]
    fn test_printer_changes_new() {
        let changes = PrinterChanges::new("Test Printer".to_string());

        assert_eq!(changes.printer_name, "Test Printer");
        assert!(!changes.has_changes());
        assert_eq!(changes.change_count(), 0);
    }

    #[test]
    fn test_printer_changes_has_property_change() {
        let mut changes = PrinterChanges::new("Test Printer".to_string());

        changes.changes.push(PropertyChange::IsOffline {
            old: false,
            new: true,
        });

        assert!(changes.has_changes());
        assert_eq!(changes.change_count(), 1);
        assert!(changes.has_property_change("IsOffline"));
        assert!(!changes.has_property_change("Status"));
    }

    #[test]
    fn test_printer_compare_with_no_changes() {
        let printer1 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let printer2 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let changes = printer1.compare_with(&printer2);
        assert!(!changes.has_changes());
    }

    #[test]
    fn test_printer_compare_with_status_change() {
        let printer1 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let printer2 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Printing,
            ErrorState::NoError,
            false,
            true,
        );

        let changes = printer1.compare_with(&printer2);
        assert!(changes.has_changes());
        assert_eq!(changes.change_count(), 1);
        assert!(changes.has_property_change("Status"));
    }

    #[test]
    fn test_printer_compare_with_multiple_changes() {
        let printer1 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let printer2 = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Printing,
            ErrorState::Jammed,
            true,
            false,
        );

        let changes = printer1.compare_with(&printer2);
        assert!(changes.has_changes());
        assert!(changes.change_count() >= 4); // Status, ErrorState, IsOffline, IsDefault
        assert!(changes.has_property_change("Status"));
        assert!(changes.has_property_change("ErrorState"));
        assert!(changes.has_property_change("IsOffline"));
        assert!(changes.has_property_change("IsDefault"));
    }

    #[test]
    fn test_error_state_descriptions() {
        assert_eq!(ErrorState::NoError.description(), "No Error");
        assert_eq!(ErrorState::Jammed.description(), "Jammed");
        assert_eq!(ErrorState::NoPaper.description(), "No Paper");
        assert_eq!(ErrorState::LowPaper.description(), "Low Paper");
        assert_eq!(ErrorState::NoToner.description(), "No Toner");
        assert_eq!(ErrorState::DoorOpen.description(), "Door Open");
    }

    #[test]
    fn test_printer_status_descriptions() {
        assert_eq!(PrinterStatus::Idle.description(), "Idle");
        assert_eq!(PrinterStatus::Printing.description(), "Printing");
        assert_eq!(PrinterStatus::Offline.description(), "Offline");
        assert_eq!(PrinterStatus::Warmup.description(), "Warming Up");
    }

    #[test]
    fn test_printer_state_descriptions() {
        assert_eq!(PrinterState::PaperJam.description(), "Paper Jam");
        assert_eq!(PrinterState::TonerLow.description(), "Toner Low");
        assert_eq!(PrinterState::Printing.description(), "Printing");
        assert_eq!(PrinterState::DoorOpen.description(), "Door Open");
    }

    #[cfg(windows)]
    #[test]
    fn test_printer_status_from_u32() {
        assert_eq!(PrinterStatus::from_u32(Some(1)), PrinterStatus::Other);
        assert_eq!(PrinterStatus::from_u32(Some(2)), PrinterStatus::Unknown);
        assert_eq!(PrinterStatus::from_u32(Some(3)), PrinterStatus::Idle);
        assert_eq!(PrinterStatus::from_u32(Some(4)), PrinterStatus::Printing);
        assert_eq!(PrinterStatus::from_u32(Some(5)), PrinterStatus::Warmup);
        assert_eq!(
            PrinterStatus::from_u32(Some(6)),
            PrinterStatus::StoppedPrinting
        );
        assert_eq!(PrinterStatus::from_u32(Some(7)), PrinterStatus::Offline);
        assert_eq!(
            PrinterStatus::from_u32(Some(99)),
            PrinterStatus::StatusUnknown
        );
        assert_eq!(PrinterStatus::from_u32(None), PrinterStatus::StatusUnknown);
    }

    #[cfg(windows)]
    #[test]
    fn test_printer_state_from_u32_bitwise_flags() {
        // Test individual flags
        assert_eq!(PrinterState::from_u32(0), PrinterState::None);
        assert_eq!(PrinterState::from_u32(1), PrinterState::Paused);
        assert_eq!(PrinterState::from_u32(2), PrinterState::Error);
        assert_eq!(PrinterState::from_u32(8), PrinterState::PaperJam);
        assert_eq!(PrinterState::from_u32(1024), PrinterState::Printing);
        assert_eq!(PrinterState::from_u32(16384), PrinterState::Processing);

        // Generic Error still wins over operational states when no specific
        // cause is set - this preserves "something is wrong" signalling.
        assert_eq!(PrinterState::from_u32(2 | 1024), PrinterState::Error);

        // DoorOpen is the highest-priority specific error.
        assert_eq!(PrinterState::from_u32(4194304 | 2), PrinterState::DoorOpen);

        // B1 regression guard: specific error bits MUST win over the generic
        // `Error` bit, because WMI typically OR's them together and the
        // specific cause is more informative than the umbrella label.
        assert_eq!(PrinterState::from_u32(2 | 8), PrinterState::PaperJam);
        assert_eq!(PrinterState::from_u32(2 | 16), PrinterState::PaperOut);
        assert_eq!(PrinterState::from_u32(2 | 262_144), PrinterState::NoToner);
        assert_eq!(
            PrinterState::from_u32(2 | 2_097_152),
            PrinterState::OutOfMemory
        );

        // Offline/unreachable preferred over generic Error too.
        assert_eq!(PrinterState::from_u32(2 | 128), PrinterState::Offline);
    }

    #[cfg(windows)]
    #[test]
    fn test_error_state_from_u32() {
        assert_eq!(ErrorState::from_u32(Some(0)), ErrorState::NoError);
        assert_eq!(ErrorState::from_u32(Some(1)), ErrorState::Other);
        assert_eq!(ErrorState::from_u32(Some(2)), ErrorState::NoError);
        assert_eq!(ErrorState::from_u32(Some(3)), ErrorState::LowPaper);
        assert_eq!(ErrorState::from_u32(Some(4)), ErrorState::NoPaper);
        assert_eq!(ErrorState::from_u32(Some(5)), ErrorState::LowToner);
        assert_eq!(ErrorState::from_u32(Some(6)), ErrorState::NoToner);
        assert_eq!(ErrorState::from_u32(Some(7)), ErrorState::DoorOpen);
        assert_eq!(ErrorState::from_u32(Some(8)), ErrorState::Jammed);
        assert_eq!(ErrorState::from_u32(Some(10)), ErrorState::ServiceRequested);
        assert_eq!(ErrorState::from_u32(Some(11)), ErrorState::OutputBinFull);
        assert_eq!(ErrorState::from_u32(Some(99)), ErrorState::UnknownError);
        assert_eq!(ErrorState::from_u32(None), ErrorState::UnknownError);
    }

    #[test]
    fn test_is_offline_cannot_drift_below_status() {
        // B9 regression guard: passing `is_offline=false` MUST NOT silently
        // contradict a status of Offline.
        let printer = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Offline,
            ErrorState::NoError,
            false,
            false,
        );
        assert!(printer.is_offline());

        // Same via `new_with_state` with an unreachable state.
        let printer = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            Some(PrinterState::Offline),
            ErrorState::NoError,
            false,
            false,
        );
        assert!(printer.is_offline());

        // Hint=true wins by itself.
        let printer = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            true,
            false,
        );
        assert!(printer.is_offline());

        // All-consistent online state stays online.
        let printer = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            Some(PrinterState::Printing),
            ErrorState::NoError,
            false,
            false,
        );
        assert!(!printer.is_offline());
    }

    #[test]
    fn test_enums_hashable_for_collections() {
        use std::collections::HashSet;

        // PrinterStatus, PrinterState, ErrorState should all be usable in HashSet/HashMap.
        let mut statuses: HashSet<PrinterStatus> = HashSet::new();
        statuses.insert(PrinterStatus::Idle);
        statuses.insert(PrinterStatus::Printing);
        statuses.insert(PrinterStatus::Idle);
        assert_eq!(statuses.len(), 2);

        let mut states: HashSet<PrinterState> = HashSet::new();
        states.insert(PrinterState::Printing);
        states.insert(PrinterState::PaperJam);
        assert_eq!(states.len(), 2);

        let mut errors: HashSet<ErrorState> = HashSet::new();
        errors.insert(ErrorState::NoError);
        errors.insert(ErrorState::Jammed);
        errors.insert(ErrorState::NoError);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_changes_summary() {
        let mut changes = PrinterChanges::new("Test Printer".to_string());

        assert_eq!(changes.summary(), "No changes detected");

        changes.changes.push(PropertyChange::Status {
            old: PrinterStatus::Idle,
            new: PrinterStatus::Printing,
        });

        changes.changes.push(PropertyChange::IsOffline {
            old: false,
            new: true,
        });

        let summary = changes.summary();
        assert!(summary.contains("2 properties changed"));
        assert!(summary.contains("Status"));
        assert!(summary.contains("IsOffline"));
    }
}
