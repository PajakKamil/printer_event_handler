//! Print job representation - mirrors WMI's `Win32_PrintJob`.
//!
//! The Linux backend currently returns an empty job list (deferred Linux
//! work). On Windows, [`Job`] is populated from `Win32_PrintJob` rows and
//! [`JobStatus`] is parsed from the documented bitmask values.

#[cfg(windows)]
use serde::Deserialize;

// Win32_PrintJob.JobStatus bitmask values (documented at
// https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-printjob).
// Used by `JobStatus::from_u32`. Kept `pub(crate)` so backend code can also
// reference them if needed.
pub(crate) const JOB_STATUS_PAUSED: u32 = 0x0000_0001;
pub(crate) const JOB_STATUS_ERROR: u32 = 0x0000_0002;
pub(crate) const JOB_STATUS_DELETING: u32 = 0x0000_0004;
pub(crate) const JOB_STATUS_SPOOLING: u32 = 0x0000_0008;
pub(crate) const JOB_STATUS_PRINTING: u32 = 0x0000_0010;
pub(crate) const JOB_STATUS_OFFLINE: u32 = 0x0000_0020;
pub(crate) const JOB_STATUS_PAPEROUT: u32 = 0x0000_0040;
pub(crate) const JOB_STATUS_PRINTED: u32 = 0x0000_0080;
pub(crate) const JOB_STATUS_DELETED: u32 = 0x0000_0100;
pub(crate) const JOB_STATUS_BLOCKED_DEVQ: u32 = 0x0000_0200;
pub(crate) const JOB_STATUS_USER_INTERVENTION: u32 = 0x0000_0400;
pub(crate) const JOB_STATUS_RESTART: u32 = 0x0000_0800;
pub(crate) const JOB_STATUS_COMPLETE: u32 = 0x0000_1000;

/// High-level status of a print job, parsed from the
/// `Win32_PrintJob.JobStatus` bitmask.
///
/// Multiple bits may be set simultaneously (a `Paused` job that also hit an
/// `Error` for example); [`Self::from_u32`] resolves the most informative
/// single variant using a priority chain similar to `PrinterState::from_u32`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum JobStatus {
    None,
    Paused,
    Error,
    Deleting,
    Spooling,
    Printing,
    Offline,
    PaperOut,
    Printed,
    Deleted,
    BlockedDeviceQueue,
    UserIntervention,
    Restart,
    Complete,
    Unknown,
}

impl JobStatus {
    /// Resolves the most informative single status variant from a
    /// `Win32_PrintJob.JobStatus` bitmask. Specific causes win over the
    /// generic `Error` bit.
    #[cfg(windows)]
    pub(crate) fn from_u32(status: u32) -> Self {
        if status == 0 {
            return JobStatus::None;
        }
        if status & JOB_STATUS_PAPEROUT != 0 {
            JobStatus::PaperOut
        } else if status & JOB_STATUS_USER_INTERVENTION != 0 {
            JobStatus::UserIntervention
        } else if status & JOB_STATUS_BLOCKED_DEVQ != 0 {
            JobStatus::BlockedDeviceQueue
        } else if status & JOB_STATUS_OFFLINE != 0 {
            JobStatus::Offline
        } else if status & JOB_STATUS_ERROR != 0 {
            JobStatus::Error
        } else if status & JOB_STATUS_DELETING != 0 {
            JobStatus::Deleting
        } else if status & JOB_STATUS_DELETED != 0 {
            JobStatus::Deleted
        } else if status & JOB_STATUS_RESTART != 0 {
            JobStatus::Restart
        } else if status & JOB_STATUS_PAUSED != 0 {
            JobStatus::Paused
        } else if status & JOB_STATUS_PRINTING != 0 {
            JobStatus::Printing
        } else if status & JOB_STATUS_SPOOLING != 0 {
            JobStatus::Spooling
        } else if status & JOB_STATUS_COMPLETE != 0 {
            JobStatus::Complete
        } else if status & JOB_STATUS_PRINTED != 0 {
            JobStatus::Printed
        } else {
            JobStatus::Unknown
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            JobStatus::None => "None",
            JobStatus::Paused => "Paused",
            JobStatus::Error => "Error",
            JobStatus::Deleting => "Deleting",
            JobStatus::Spooling => "Spooling",
            JobStatus::Printing => "Printing",
            JobStatus::Offline => "Offline",
            JobStatus::PaperOut => "Paper Out",
            JobStatus::Printed => "Printed",
            JobStatus::Deleted => "Deleted",
            JobStatus::BlockedDeviceQueue => "Blocked Device Queue",
            JobStatus::UserIntervention => "User Intervention Required",
            JobStatus::Restart => "Restart",
            JobStatus::Complete => "Complete",
            JobStatus::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Internal WMI shape for a `Win32_PrintJob` row. Mirrors how `Win32Printer`
/// is structured for `Win32_Printer`.
#[cfg(windows)]
#[derive(Deserialize, Debug)]
pub(crate) struct Win32PrintJob {
    #[serde(rename = "JobId")]
    pub job_id: Option<u32>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "Document")]
    pub document: Option<String>,
    #[serde(rename = "Status")]
    pub status: Option<String>,
    #[serde(rename = "JobStatus")]
    pub job_status: Option<u32>,
    #[serde(rename = "TotalPages")]
    pub total_pages: Option<u32>,
    #[serde(rename = "PagesPrinted")]
    pub pages_printed: Option<u32>,
    #[serde(rename = "TimeSubmitted")]
    pub time_submitted: Option<String>,
    #[serde(rename = "Owner")]
    pub owner: Option<String>,
    #[serde(rename = "PrinterName")]
    pub printer_name: Option<String>,
}

/// A print job queued or in-flight on a printer.
///
/// Constructed by [`crate::backend::PrinterBackend::list_jobs`]. The raw
/// `WMI` bitmask is preserved alongside the parsed [`JobStatus`] so callers
/// who need exact platform values can read [`Self::job_status_code`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Job {
    job_id: u32,
    name: Option<String>,
    document: Option<String>,
    status_string: Option<String>,
    status: JobStatus,
    job_status_code: Option<u32>,
    total_pages: Option<u32>,
    pages_printed: Option<u32>,
    /// WMI `TimeSubmitted` in its native CIM datetime format
    /// (`YYYYMMDDHHMMSS.MMMMMM+UUU`). Parsing into a `chrono::DateTime` is
    /// deferred - callers who need a typed timestamp can parse this string
    /// themselves. Unset on backends that don't expose submit time.
    time_submitted_raw: Option<String>,
    owner: Option<String>,
    printer_name: Option<String>,
}

impl Job {
    /// Constructs a [`Job`] explicitly. Most callers should rely on
    /// [`crate::backend::PrinterBackend::list_jobs`] - this constructor
    /// exists for tests and synthetic backends.
    pub fn new(job_id: u32, printer_name: Option<String>, status: JobStatus) -> Self {
        Self {
            job_id,
            name: None,
            document: None,
            status_string: None,
            status,
            job_status_code: None,
            total_pages: None,
            pages_printed: None,
            time_submitted_raw: None,
            owner: None,
            printer_name,
        }
    }

    pub fn job_id(&self) -> u32 {
        self.job_id
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn document(&self) -> Option<&str> {
        self.document.as_deref()
    }
    pub fn status_string(&self) -> Option<&str> {
        self.status_string.as_deref()
    }
    pub fn status(&self) -> &JobStatus {
        &self.status
    }
    pub fn job_status_code(&self) -> Option<u32> {
        self.job_status_code
    }
    pub fn total_pages(&self) -> Option<u32> {
        self.total_pages
    }
    pub fn pages_printed(&self) -> Option<u32> {
        self.pages_printed
    }
    pub fn time_submitted_raw(&self) -> Option<&str> {
        self.time_submitted_raw.as_deref()
    }
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    pub fn printer_name(&self) -> Option<&str> {
        self.printer_name.as_deref()
    }
}

#[cfg(windows)]
impl From<Win32PrintJob> for Job {
    fn from(raw: Win32PrintJob) -> Self {
        let job_status_code = raw.job_status;
        let status = job_status_code
            .map(JobStatus::from_u32)
            .unwrap_or(JobStatus::Unknown);
        Self {
            job_id: raw.job_id.unwrap_or(0),
            name: raw.name,
            document: raw.document,
            status_string: raw.status,
            status,
            job_status_code,
            total_pages: raw.total_pages,
            pages_printed: raw.pages_printed,
            time_submitted_raw: raw.time_submitted,
            owner: raw.owner,
            printer_name: raw.printer_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn job_status_from_u32_priority() {
        // Specific causes win over generic Error.
        assert_eq!(
            JobStatus::from_u32(JOB_STATUS_ERROR | JOB_STATUS_PAPEROUT),
            JobStatus::PaperOut
        );
        assert_eq!(
            JobStatus::from_u32(JOB_STATUS_ERROR | JOB_STATUS_USER_INTERVENTION),
            JobStatus::UserIntervention
        );
        // Generic Error wins when no specific cause is set.
        assert_eq!(JobStatus::from_u32(JOB_STATUS_ERROR), JobStatus::Error);
        // Zero is `None`.
        assert_eq!(JobStatus::from_u32(0), JobStatus::None);
        // Operational state.
        assert_eq!(JobStatus::from_u32(JOB_STATUS_PRINTING), JobStatus::Printing);
    }

    #[test]
    fn job_new_accessors() {
        let job = Job::new(42, Some("HP LaserJet".to_string()), JobStatus::Printing);
        assert_eq!(job.job_id(), 42);
        assert_eq!(job.printer_name(), Some("HP LaserJet"));
        assert_eq!(job.status(), &JobStatus::Printing);
        assert!(job.name().is_none());
    }
}
