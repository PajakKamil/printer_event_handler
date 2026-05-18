use super::error_state::ErrorState;
use super::state::PrinterState;
use super::status::PrinterStatus;

/// Represents a change in a specific printer property
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_get_property_changes_filters_correctly() {
        let mut changes = PrinterChanges::new("Test Printer".to_string());
        changes.changes.push(PropertyChange::Status {
            old: PrinterStatus::Idle,
            new: PrinterStatus::Printing,
        });
        changes.changes.push(PropertyChange::IsOffline {
            old: false,
            new: true,
        });
        changes.changes.push(PropertyChange::ErrorState {
            old: ErrorState::NoError,
            new: ErrorState::Jammed,
        });

        assert_eq!(changes.get_property_changes("Status").len(), 1);
        assert_eq!(changes.get_property_changes("IsOffline").len(), 1);
        assert_eq!(changes.get_property_changes("ErrorState").len(), 1);
        assert_eq!(changes.get_property_changes("Name").len(), 0);
    }

    #[test]
    fn test_property_change_descriptions_are_non_empty() {
        // Smoke test: every PropertyChange variant produces a non-empty
        // description, so logging code paths can format any change safely.
        let samples = vec![
            PropertyChange::Name {
                old: "a".to_string(),
                new: "b".to_string(),
            },
            PropertyChange::Status {
                old: PrinterStatus::Idle,
                new: PrinterStatus::Printing,
            },
            PropertyChange::State {
                old: None,
                new: Some(PrinterState::Printing),
            },
            PropertyChange::ErrorState {
                old: ErrorState::NoError,
                new: ErrorState::Jammed,
            },
            PropertyChange::IsOffline {
                old: false,
                new: true,
            },
            PropertyChange::IsDefault {
                old: false,
                new: true,
            },
            PropertyChange::PrinterStatusCode {
                old: Some(3),
                new: Some(4),
            },
            PropertyChange::PrinterStateCode {
                old: Some(0),
                new: Some(1024),
            },
            PropertyChange::DetectedErrorStateCode {
                old: Some(2),
                new: Some(8),
            },
            PropertyChange::ExtendedDetectedErrorStateCode {
                old: Some(2),
                new: Some(8),
            },
            PropertyChange::ExtendedPrinterStatusCode {
                old: Some(3),
                new: Some(7),
            },
            PropertyChange::WmiStatus {
                old: Some("OK".to_string()),
                new: Some("Degraded".to_string()),
            },
        ];

        for change in samples {
            assert!(
                !change.property_name().is_empty(),
                "property_name should not be empty for {:?}",
                change
            );
            assert!(
                !change.description().is_empty(),
                "description should not be empty for {:?}",
                change
            );
        }
    }
}
