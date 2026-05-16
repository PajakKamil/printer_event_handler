use crate::{ErrorState, PrinterStatus};

/// Summary information about a printer's current state.
///
/// This struct provides a snapshot of a printer's essential status information
/// in a convenient format for reporting and monitoring applications.
#[derive(Debug, Clone)]
pub struct PrinterSummary {
    /// Current operational status of the printer
    pub status: PrinterStatus,
    /// Current error state of the printer
    pub error_state: ErrorState,
    /// Whether the printer is currently offline
    pub is_offline: bool,
    /// Whether this is the system's default printer
    pub is_default: bool,
    /// Whether the printer currently has any error conditions
    pub has_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_summary_structure() {
        let summary = PrinterSummary {
            status: PrinterStatus::Idle,
            error_state: ErrorState::NoError,
            is_offline: false,
            is_default: true,
            has_error: false,
        };

        assert_eq!(summary.status, PrinterStatus::Idle);
        assert_eq!(summary.error_state, ErrorState::NoError);
        assert!(!summary.is_offline);
        assert!(summary.is_default);
        assert!(!summary.has_error);
    }
}
