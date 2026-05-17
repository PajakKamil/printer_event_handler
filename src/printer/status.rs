/// Represents a printer's status (Win32_Printer.PrinterStatus - Current/Recommended)
///
/// This is the current WMI property for printer status information.
/// Values 1-7 according to Microsoft documentation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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

impl std::fmt::Display for PrinterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
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
    fn test_printer_status_descriptions() {
        assert_eq!(PrinterStatus::Idle.description(), "Idle");
        assert_eq!(PrinterStatus::Printing.description(), "Printing");
        assert_eq!(PrinterStatus::Offline.description(), "Offline");
        assert_eq!(PrinterStatus::Warmup.description(), "Warming Up");
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
}
