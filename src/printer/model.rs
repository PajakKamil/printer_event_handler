#[cfg(windows)]
use serde::Deserialize;

use super::change::{PrinterChanges, PropertyChange};
use super::error_state::ErrorState;
use super::state::PrinterState;
use super::status::PrinterStatus;

#[cfg(windows)]
use super::state::{
    PRINTER_STATE_DOOR_OPEN, PRINTER_STATE_ERROR, PRINTER_STATE_INITIALIZING,
    PRINTER_STATE_NO_TONER, PRINTER_STATE_NOT_AVAILABLE, PRINTER_STATE_OFFLINE,
    PRINTER_STATE_OUT_OF_MEMORY, PRINTER_STATE_OUTPUT_BIN_FULL, PRINTER_STATE_PAGE_PUNT,
    PRINTER_STATE_PAUSED, PRINTER_STATE_POWER_SAVE, PRINTER_STATE_PRINTING,
    PRINTER_STATE_PROCESSING, PRINTER_STATE_SERVER_UNKNOWN, PRINTER_STATE_TONER_LOW,
    PRINTER_STATE_USER_INTERVENTION_REQUIRED, PRINTER_STATE_WAITING, PRINTER_STATE_WARMING_UP,
};

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
    #[cfg(windows)]
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

    /// Returns human-readable description of PrinterState code (obsolete property).
    ///
    /// Non-Windows stub - the obsolete WMI PrinterState property is Windows-only.
    /// Always returns `None` on non-Windows platforms because
    /// [`Printer::printer_state_code`] is always `None` there.
    #[cfg(not(windows))]
    pub fn printer_state_description(&self) -> Option<&'static str> {
        None
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

    // --- Observation: compare_with isolates a single property correctly ---

    fn base_printer() -> Printer {
        Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        )
    }

    #[test]
    fn test_compare_with_only_name_change() {
        let a = base_printer();
        let b = Printer::new(
            "Renamed".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            true,
        );

        let changes = a.compare_with(&b);
        assert_eq!(changes.change_count(), 1);
        assert!(matches!(changes.changes[0], PropertyChange::Name { .. }));
    }

    #[test]
    fn test_compare_with_only_error_state_change() {
        let a = base_printer();
        let b = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::Jammed,
            false,
            true,
        );

        let changes = a.compare_with(&b);
        assert_eq!(changes.change_count(), 1);
        assert!(changes.has_property_change("ErrorState"));
    }

    #[test]
    fn test_compare_with_only_is_default_change() {
        let a = base_printer();
        let b = Printer::new(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            ErrorState::NoError,
            false,
            false,
        );

        let changes = a.compare_with(&b);
        assert_eq!(changes.change_count(), 1);
        assert!(changes.has_property_change("IsDefault"));
    }

    #[test]
    fn test_compare_with_only_is_offline_change() {
        // Use new_with_state so we can flip is_offline without also flipping
        // status (status::Offline would imply is_offline via derive_is_offline).
        let a = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            true,
        );
        let b = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            true,
            true,
        );

        let changes = a.compare_with(&b);
        assert_eq!(changes.change_count(), 1);
        assert!(matches!(
            changes.changes[0],
            PropertyChange::IsOffline {
                old: false,
                new: true
            }
        ));
    }

    #[test]
    fn test_compare_with_only_state_field_change() {
        let a = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            Some(PrinterState::None),
            ErrorState::NoError,
            false,
            true,
        );
        let b = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            Some(PrinterState::Paused),
            ErrorState::NoError,
            false,
            true,
        );

        let changes = a.compare_with(&b);
        assert_eq!(changes.change_count(), 1);
        assert!(changes.has_property_change("State"));
    }

    #[test]
    fn test_compare_with_state_appearance() {
        // Going from "no state info" to "some state" must surface as a State
        // change. Regression guard for monitors that only diff Status.
        let a = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            true,
        );
        let b = Printer::new_with_state(
            "Test Printer".to_string(),
            PrinterStatus::Idle,
            Some(PrinterState::Printing),
            ErrorState::NoError,
            false,
            true,
        );

        let changes = a.compare_with(&b);
        assert!(changes.has_property_change("State"));
        let state_changes: Vec<&PropertyChange> = changes.get_property_changes("State");
        assert_eq!(state_changes.len(), 1);
        if let PropertyChange::State { old, new } = state_changes[0] {
            assert!(old.is_none());
            assert_eq!(new.as_ref(), Some(&PrinterState::Printing));
        } else {
            panic!("expected PropertyChange::State");
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_compare_with_raw_wmi_codes_change() {
        // Round-trip two Win32Printer payloads through `From<Win32Printer>`
        // so we exercise the same Printer construction path that
        // WindowsBackend uses. The only diff is the raw code values.
        let make = |status: u32, error: u32| Win32Printer {
            name: Some("Test Printer".to_string()),
            printer_status: Some(status),
            detected_error_state: Some(error),
            work_offline: Some(false),
            printer_state: Some(0),
            default: Some(false),
            extended_printer_status: Some(status),
            extended_detected_error_state: Some(error),
            status: Some("OK".to_string()),
        };

        let a: Printer = make(3, 2).into(); // Idle, NoError
        let b: Printer = make(4, 8).into(); // Printing, Jammed

        let changes = a.compare_with(&b);
        // Expect: Status, ErrorState, PrinterStatusCode,
        // DetectedErrorStateCode, ExtendedPrinterStatusCode,
        // ExtendedDetectedErrorStateCode.
        assert!(changes.has_property_change("PrinterStatusCode"));
        assert!(changes.has_property_change("DetectedErrorStateCode"));
        assert!(changes.has_property_change("ExtendedPrinterStatusCode"));
        assert!(changes.has_property_change("ExtendedDetectedErrorStateCode"));
        assert!(changes.has_property_change("Status"));
        assert!(changes.has_property_change("ErrorState"));
    }

    #[cfg(windows)]
    #[test]
    fn test_compare_with_only_wmi_status_string_changes() {
        let make = |wmi_status: &str| Win32Printer {
            name: Some("Test Printer".to_string()),
            printer_status: Some(3),
            detected_error_state: Some(2),
            work_offline: Some(false),
            printer_state: Some(0),
            default: Some(false),
            extended_printer_status: Some(3),
            extended_detected_error_state: Some(2),
            status: Some(wmi_status.to_string()),
        };

        let a: Printer = make("OK").into();
        let b: Printer = make("Degraded").into();

        let changes = a.compare_with(&b);
        // WmiStatus must change.
        assert!(changes.has_property_change("WmiStatus"));
        // "Degraded" flips is_offline via the WMI-status fallback, so
        // IsOffline is expected to show up too.
        assert!(changes.has_property_change("IsOffline"));
    }
}
