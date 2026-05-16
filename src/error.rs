use std::fmt;

/// Errors that can occur when working with printers
#[derive(Debug)]
pub enum PrinterError {
    /// WMI connection or query failed
    WmiError(String),
    /// CUPS connection or query failed
    CupsError(String),
    /// Printer was not found
    PrinterNotFound(String),
    /// Platform not supported
    PlatformNotSupported,
    /// General I/O error
    IoError(std::io::Error),
    /// Other errors
    Other(String),
}

impl fmt::Display for PrinterError {
    /// Formats the error for display to users
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrinterError::WmiError(msg) => write!(f, "WMI error: {}", msg),
            PrinterError::CupsError(msg) => write!(f, "CUPS error: {}", msg),
            PrinterError::PrinterNotFound(name) => write!(f, "Printer '{}' not found", name),
            PrinterError::PlatformNotSupported => {
                write!(f, "This platform is not supported")
            }
            PrinterError::IoError(err) => write!(f, "I/O error: {}", err),
            PrinterError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PrinterError {
    /// Returns the source of this error, if any
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PrinterError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PrinterError {
    /// Converts std::io::Error into PrinterError
    fn from(err: std::io::Error) -> Self {
        PrinterError::IoError(err)
    }
}

#[cfg(windows)]
impl From<wmi::WMIError> for PrinterError {
    /// Converts WMI errors into PrinterError (Windows only)
    fn from(err: wmi::WMIError) -> Self {
        PrinterError::WmiError(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for PrinterError {
    /// Converts boxed errors into PrinterError
    fn from(err: Box<dyn std::error::Error>) -> Self {
        PrinterError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_wmi_error_display() {
        let err = PrinterError::WmiError("Connection failed".to_string());
        assert_eq!(err.to_string(), "WMI error: Connection failed");
    }

    #[test]
    fn test_cups_error_display() {
        let err = PrinterError::CupsError("CUPS not available".to_string());
        assert_eq!(err.to_string(), "CUPS error: CUPS not available");
    }

    #[test]
    fn test_printer_not_found_display() {
        let err = PrinterError::PrinterNotFound("HP LaserJet".to_string());
        assert_eq!(err.to_string(), "Printer 'HP LaserJet' not found");
    }

    #[test]
    fn test_platform_not_supported_display() {
        let err = PrinterError::PlatformNotSupported;
        assert_eq!(err.to_string(), "This platform is not supported");
    }

    #[test]
    fn test_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = PrinterError::IoError(io_err);
        assert!(err.to_string().contains("I/O error"));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_other_error_display() {
        let err = PrinterError::Other("Custom error message".to_string());
        assert_eq!(err.to_string(), "Custom error message");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let printer_err: PrinterError = io_err.into();

        match printer_err {
            PrinterError::IoError(_) => {} // Expected
            _ => panic!("Should convert to IoError variant"),
        }
    }

    #[test]
    fn test_from_boxed_error() {
        let boxed_err: Box<dyn std::error::Error> =
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, "test error"));
        let printer_err: PrinterError = boxed_err.into();

        match printer_err {
            PrinterError::Other(msg) => {
                assert!(msg.contains("test error"));
            }
            _ => panic!("Should convert to Other variant"),
        }
    }

    #[test]
    fn test_error_source_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err = PrinterError::IoError(io_err);

        // IoError should have a source
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_source_other_errors() {
        // Other error types shouldn't have a source
        assert!(
            PrinterError::WmiError("test".to_string())
                .source()
                .is_none()
        );
        assert!(
            PrinterError::CupsError("test".to_string())
                .source()
                .is_none()
        );
        assert!(
            PrinterError::PrinterNotFound("test".to_string())
                .source()
                .is_none()
        );
        assert!(PrinterError::PlatformNotSupported.source().is_none());
        assert!(PrinterError::Other("test".to_string()).source().is_none());
    }

    #[test]
    fn test_error_debug_format() {
        let err = PrinterError::WmiError("test error".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("WmiError"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_all_error_variants_are_error_trait() {
        // Test that all variants implement std::error::Error
        fn is_error<E: std::error::Error>(_: &E) {}

        is_error(&PrinterError::WmiError("test".to_string()));
        is_error(&PrinterError::CupsError("test".to_string()));
        is_error(&PrinterError::PrinterNotFound("test".to_string()));
        is_error(&PrinterError::PlatformNotSupported);
        is_error(&PrinterError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "test",
        )));
        is_error(&PrinterError::Other("test".to_string()));
    }
}
