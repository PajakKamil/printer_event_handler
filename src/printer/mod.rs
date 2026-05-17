//! Printer domain types: status/state/error enums, the [`Printer`] aggregate,
//! and the [`PropertyChange`]/[`PrinterChanges`] diff types used by the
//! monitor.
//!
//! Public surface re-exported at the crate root - external callers should
//! continue to use `printer_event_handler::PrinterStatus` etc. Submodules
//! exist for internal organization only.

mod change;
mod error_state;
mod job;
mod model;
pub(crate) mod state;
mod status;

pub use change::{PrinterChanges, PropertyChange};
pub use error_state::ErrorState;
pub use job::{Job, JobStatus};
pub use model::Printer;
pub use state::PrinterState;
pub use status::PrinterStatus;

#[cfg(windows)]
pub use model::WmiStatusCodes;

// Internal-to-crate re-export so `crate::backend` can name `Win32Printer`
// via `crate::printer::Win32Printer` exactly as before the split.
#[cfg(windows)]
pub(crate) use model::Win32Printer;
#[cfg(windows)]
pub(crate) use job::Win32PrintJob;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_enums_hashable_for_collections() {
        // Cross-cutting smoke test: PrinterStatus, PrinterState, ErrorState
        // should all be usable in HashSet/HashMap. Lives here because it
        // covers all three enums together rather than any single one.
        let mut statuses: HashSet<PrinterStatus> = HashSet::new();
        statuses.insert(PrinterStatus::Idle);
        statuses.insert(PrinterStatus::Printing);
        statuses.insert(PrinterStatus::Idle);
        assert_eq!(statuses.len(), 2);

        let mut states: HashSet<PrinterState> = HashSet::new();
        states.insert(PrinterState::Printing);
        states.insert(PrinterState::PaperJam);
        assert_eq!(states.len(), 2);

        let mut errors: HashSet<ErrorState> = HashSet::new();
        errors.insert(ErrorState::NoError);
        errors.insert(ErrorState::Jammed);
        errors.insert(ErrorState::NoError);
        assert_eq!(errors.len(), 2);
    }
}
