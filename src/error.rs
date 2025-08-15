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
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PrinterError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PrinterError {
    fn from(err: std::io::Error) -> Self {
        PrinterError::IoError(err)
    }
}

#[cfg(windows)]
impl From<wmi::WMIError> for PrinterError {
    fn from(err: wmi::WMIError) -> Self {
        PrinterError::WmiError(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for PrinterError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        PrinterError::Other(err.to_string())
    }
}
