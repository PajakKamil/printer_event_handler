/// Represents a printer's error state
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_state_is_error() {
        assert!(!ErrorState::NoError.is_error());
        assert!(ErrorState::Jammed.is_error());
        assert!(ErrorState::NoPaper.is_error());
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
}
