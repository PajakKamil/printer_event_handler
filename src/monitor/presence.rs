use crate::Printer;

/// Per-printer state carried across polls inside `monitor_multiple_printers`.
///
/// `snapshot` is the next-comparison baseline. After a fresh disappearance it
/// is replaced with a synthetic "missing" snapshot (Offline / UnknownError /
/// is_offline=true) so the reappearance comparison surfaces the
/// `IsOffline: true -> false` delta plus any other property differences (B4).
/// `was_present_last_poll` distinguishes a fresh disappearance from continued
/// absence, ensuring the disappearance callback fires exactly once per gap.
#[derive(Debug, Clone)]
pub(super) struct PresenceTracker {
    pub(super) snapshot: Option<Printer>,
    pub(super) was_present_last_poll: bool,
}

impl PresenceTracker {
    pub(super) fn new() -> Self {
        Self {
            snapshot: None,
            was_present_last_poll: false,
        }
    }
}
