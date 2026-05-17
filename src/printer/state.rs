use super::status::PrinterStatus;

// .NET PrintQueueStatus flag values used by Win32_Printer.PrinterState.
// Defined once here so the priority chain in `PrinterState::from_u32` and the
// description tables in `Printer::printer_state_description` stay in sync.
// `pub(super)` so sibling `model::Printer::printer_state_description` can read
// them without re-defining the constants.
// Reference: https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus
pub(super) const PRINTER_STATE_PAUSED: u32 = 1;
pub(super) const PRINTER_STATE_ERROR: u32 = 2;
pub(super) const PRINTER_STATE_PENDING_DELETION: u32 = 4;
pub(super) const PRINTER_STATE_PAPER_JAM: u32 = 8;
pub(super) const PRINTER_STATE_PAPER_OUT: u32 = 16;
pub(super) const PRINTER_STATE_MANUAL_FEED: u32 = 32;
pub(super) const PRINTER_STATE_PAPER_PROBLEM: u32 = 64;
pub(super) const PRINTER_STATE_OFFLINE: u32 = 128;
pub(super) const PRINTER_STATE_IO_ACTIVE: u32 = 256;
pub(super) const PRINTER_STATE_BUSY: u32 = 512;
pub(super) const PRINTER_STATE_PRINTING: u32 = 1024;
pub(super) const PRINTER_STATE_OUTPUT_BIN_FULL: u32 = 2048;
pub(super) const PRINTER_STATE_NOT_AVAILABLE: u32 = 4096;
pub(super) const PRINTER_STATE_WAITING: u32 = 8192;
pub(super) const PRINTER_STATE_PROCESSING: u32 = 16_384;
pub(super) const PRINTER_STATE_INITIALIZING: u32 = 32_768;
pub(super) const PRINTER_STATE_WARMING_UP: u32 = 65_536;
pub(super) const PRINTER_STATE_TONER_LOW: u32 = 131_072;
pub(super) const PRINTER_STATE_NO_TONER: u32 = 262_144;
pub(super) const PRINTER_STATE_PAGE_PUNT: u32 = 524_288;
pub(super) const PRINTER_STATE_USER_INTERVENTION_REQUIRED: u32 = 1_048_576;
pub(super) const PRINTER_STATE_OUT_OF_MEMORY: u32 = 2_097_152;
pub(super) const PRINTER_STATE_DOOR_OPEN: u32 = 4_194_304;
pub(super) const PRINTER_STATE_SERVER_UNKNOWN: u32 = 8_388_608;
pub(super) const PRINTER_STATE_POWER_SAVE: u32 = 16_777_216;

/// Represents a printer's state using .NET PrintQueueStatus flags
///
/// This enum represents the actual WMI PrinterState values which correspond to
/// the .NET System.Printing.PrintQueueStatus enumeration flags.
/// See: <https://learn.microsoft.com/en-us/dotnet/api/system.printing.printqueuestatus>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// generic `Other`. Callers should inspect [`crate::Printer::error_state`]
    /// for the specific cause (PaperJam, NoToner, etc.).
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

impl std::fmt::Display for PrinterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_state_display() {
        assert_eq!(PrinterState::PaperJam.to_string(), "Paper Jam");
        assert_eq!(PrinterState::TonerLow.to_string(), "Toner Low");
    }

    #[test]
    fn test_printer_state_descriptions() {
        assert_eq!(PrinterState::PaperJam.description(), "Paper Jam");
        assert_eq!(PrinterState::TonerLow.description(), "Toner Low");
        assert_eq!(PrinterState::Printing.description(), "Printing");
        assert_eq!(PrinterState::DoorOpen.description(), "Door Open");
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
}
