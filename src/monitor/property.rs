/// Enum representing all available printer properties that can be monitored.
///
/// This enum provides type-safe access to all printer properties that can be
/// monitored for changes, replacing string-based property names with a
/// strongly-typed interface.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MonitorableProperty {
    /// Printer name changes
    Name,
    /// PrinterStatus enum changes (recommended current status)
    Status,
    /// PrinterState enum changes (legacy Windows state)
    State,
    /// ErrorState enum changes
    ErrorState,
    /// Online/offline status changes
    IsOffline,
    /// Default printer designation changes
    IsDefault,
    /// Raw PrinterStatus code changes (1-7)
    PrinterStatusCode,
    /// Raw PrinterState code changes (.NET flags)
    PrinterStateCode,
    /// Raw DetectedErrorState code changes (0-11)
    DetectedErrorStateCode,
    /// Raw ExtendedDetectedErrorState code changes
    ExtendedDetectedErrorStateCode,
    /// Raw ExtendedPrinterStatus code changes
    ExtendedPrinterStatusCode,
    /// WMI Status property changes ("OK", "Error", etc.)
    WmiStatus,
}

impl MonitorableProperty {
    /// Returns the string representation of the property name.
    ///
    /// This matches the property names used in the PropertyChange enum.
    pub fn as_str(&self) -> &'static str {
        match self {
            MonitorableProperty::Name => "Name",
            MonitorableProperty::Status => "Status",
            MonitorableProperty::State => "State",
            MonitorableProperty::ErrorState => "ErrorState",
            MonitorableProperty::IsOffline => "IsOffline",
            MonitorableProperty::IsDefault => "IsDefault",
            MonitorableProperty::PrinterStatusCode => "PrinterStatusCode",
            MonitorableProperty::PrinterStateCode => "PrinterStateCode",
            MonitorableProperty::DetectedErrorStateCode => "DetectedErrorStateCode",
            MonitorableProperty::ExtendedDetectedErrorStateCode => "ExtendedDetectedErrorStateCode",
            MonitorableProperty::ExtendedPrinterStatusCode => "ExtendedPrinterStatusCode",
            MonitorableProperty::WmiStatus => "WmiStatus",
        }
    }

    /// Returns a human-readable description of what this property represents.
    pub fn description(&self) -> &'static str {
        match self {
            MonitorableProperty::Name => "Printer name",
            MonitorableProperty::Status => "Current printer status (recommended)",
            MonitorableProperty::State => "Printer state (legacy Windows property)",
            MonitorableProperty::ErrorState => "Current error condition",
            MonitorableProperty::IsOffline => "Online/offline status",
            MonitorableProperty::IsDefault => "Default printer designation",
            MonitorableProperty::PrinterStatusCode => "Raw printer status code (1-7)",
            MonitorableProperty::PrinterStateCode => "Raw printer state code (.NET flags)",
            MonitorableProperty::DetectedErrorStateCode => "Raw detected error state code (0-11)",
            MonitorableProperty::ExtendedDetectedErrorStateCode => "Extended error state code",
            MonitorableProperty::ExtendedPrinterStatusCode => "Extended printer status code",
            MonitorableProperty::WmiStatus => "WMI status property",
        }
    }

    /// Returns all available properties that can be monitored.
    ///
    /// Returns a `&'static` slice so callers don't pay for an allocation just
    /// to enumerate the list. Use `.iter()` / `.contains(...)` / `.len()` as
    /// you would with a `Vec` - all the slice methods are available.
    pub fn all() -> &'static [MonitorableProperty] {
        &[
            MonitorableProperty::Name,
            MonitorableProperty::Status,
            MonitorableProperty::State,
            MonitorableProperty::ErrorState,
            MonitorableProperty::IsOffline,
            MonitorableProperty::IsDefault,
            MonitorableProperty::PrinterStatusCode,
            MonitorableProperty::PrinterStateCode,
            MonitorableProperty::DetectedErrorStateCode,
            MonitorableProperty::ExtendedDetectedErrorStateCode,
            MonitorableProperty::ExtendedPrinterStatusCode,
            MonitorableProperty::WmiStatus,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitorable_property_as_str() {
        assert_eq!(MonitorableProperty::Name.as_str(), "Name");
        assert_eq!(MonitorableProperty::Status.as_str(), "Status");
        assert_eq!(MonitorableProperty::State.as_str(), "State");
        assert_eq!(MonitorableProperty::ErrorState.as_str(), "ErrorState");
        assert_eq!(MonitorableProperty::IsOffline.as_str(), "IsOffline");
        assert_eq!(MonitorableProperty::IsDefault.as_str(), "IsDefault");
        assert_eq!(
            MonitorableProperty::PrinterStatusCode.as_str(),
            "PrinterStatusCode"
        );
        assert_eq!(
            MonitorableProperty::PrinterStateCode.as_str(),
            "PrinterStateCode"
        );
        assert_eq!(
            MonitorableProperty::DetectedErrorStateCode.as_str(),
            "DetectedErrorStateCode"
        );
        assert_eq!(
            MonitorableProperty::ExtendedDetectedErrorStateCode.as_str(),
            "ExtendedDetectedErrorStateCode"
        );
        assert_eq!(
            MonitorableProperty::ExtendedPrinterStatusCode.as_str(),
            "ExtendedPrinterStatusCode"
        );
        assert_eq!(MonitorableProperty::WmiStatus.as_str(), "WmiStatus");
    }

    #[test]
    fn test_monitorable_property_description() {
        assert_eq!(MonitorableProperty::Name.description(), "Printer name");
        assert_eq!(
            MonitorableProperty::Status.description(),
            "Current printer status (recommended)"
        );
        assert_eq!(
            MonitorableProperty::State.description(),
            "Printer state (legacy Windows property)"
        );
        assert_eq!(
            MonitorableProperty::IsOffline.description(),
            "Online/offline status"
        );
    }

    #[test]
    fn test_monitorable_property_all() {
        let all = MonitorableProperty::all();
        assert_eq!(all.len(), 12);

        // Verify all variants are present
        assert!(all.contains(&MonitorableProperty::Name));
        assert!(all.contains(&MonitorableProperty::Status));
        assert!(all.contains(&MonitorableProperty::State));
        assert!(all.contains(&MonitorableProperty::ErrorState));
        assert!(all.contains(&MonitorableProperty::IsOffline));
        assert!(all.contains(&MonitorableProperty::IsDefault));
        assert!(all.contains(&MonitorableProperty::PrinterStatusCode));
        assert!(all.contains(&MonitorableProperty::PrinterStateCode));
        assert!(all.contains(&MonitorableProperty::DetectedErrorStateCode));
        assert!(all.contains(&MonitorableProperty::ExtendedDetectedErrorStateCode));
        assert!(all.contains(&MonitorableProperty::ExtendedPrinterStatusCode));
        assert!(all.contains(&MonitorableProperty::WmiStatus));
    }

    #[test]
    fn test_monitorable_property_equality() {
        assert_eq!(MonitorableProperty::Status, MonitorableProperty::Status);
        assert_ne!(MonitorableProperty::Status, MonitorableProperty::State);
    }
}
