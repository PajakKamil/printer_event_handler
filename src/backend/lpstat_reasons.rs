//! IPP `printer-state-reasons` → domain mapping for the Linux/CUPS backend.
//!
//! CUPS exposes the IPP `printer-state-reasons` attribute (RFC 8011 §5.4.12)
//! through `lpstat -l -p`'s `Alerts:` continuation line as a comma-separated
//! list of lowercase tokens such as `media-empty-warning`, `toner-low`,
//! `cover-open-error`. This module maps that token list into the two domain
//! shapes the rest of the crate already understands:
//!
//! - an [`ErrorState`] enum value (the single most-specific cause);
//! - an OR'd bitmask of `PRINTER_STATE_*` constants (.NET PrintQueueStatus
//!   flags - same bit layout `WindowsBackend` already feeds through
//!   [`PrinterState::from_u32`]).
//!
//! Sharing the `PRINTER_STATE_*` bit layout means callers reading a `Printer`
//! cannot tell which backend produced the value - the surface is identical.

#![cfg(unix)]

use crate::ErrorState;
use crate::printer::state::{
    PRINTER_STATE_DOOR_OPEN, PRINTER_STATE_NO_TONER, PRINTER_STATE_OFFLINE,
    PRINTER_STATE_OUT_OF_MEMORY, PRINTER_STATE_OUTPUT_BIN_FULL, PRINTER_STATE_PAPER_JAM,
    PRINTER_STATE_PAPER_OUT, PRINTER_STATE_PAPER_PROBLEM, PRINTER_STATE_PAUSED,
    PRINTER_STATE_TONER_LOW,
};

/// IPP token substrings, ordered most-specific-first. The first match wins so
/// that e.g. a co-reported `media-jam` + `media-empty` surfaces as `Jammed`
/// rather than `NoPaper` - matching the priority chain used by
/// [`crate::PrinterState::from_u32`].
///
/// Token suffix conventions per RFC 8011: each reason may carry a severity
/// suffix (`-report`, `-warning`, `-error`). We match on the bare stem with
/// `starts_with` so all three severities are handled uniformly.
const REASON_RULES: &[(&str, ErrorState, u32)] = &[
    // Specific-cause errors (priority-ordered like PrinterState::from_u32).
    ("media-jam", ErrorState::Jammed, PRINTER_STATE_PAPER_JAM),
    ("door-open", ErrorState::DoorOpen, PRINTER_STATE_DOOR_OPEN),
    ("cover-open", ErrorState::DoorOpen, PRINTER_STATE_DOOR_OPEN),
    (
        "interlock-open",
        ErrorState::DoorOpen,
        PRINTER_STATE_DOOR_OPEN,
    ),
    ("media-empty", ErrorState::NoPaper, PRINTER_STATE_PAPER_OUT),
    ("media-needed", ErrorState::NoPaper, PRINTER_STATE_PAPER_OUT),
    (
        "media-low",
        ErrorState::LowPaper,
        PRINTER_STATE_PAPER_PROBLEM,
    ),
    (
        "toner-empty",
        ErrorState::NoToner,
        PRINTER_STATE_NO_TONER | PRINTER_STATE_TONER_LOW,
    ),
    ("toner-low", ErrorState::LowToner, PRINTER_STATE_TONER_LOW),
    (
        "marker-supply-empty",
        ErrorState::NoToner,
        PRINTER_STATE_NO_TONER | PRINTER_STATE_TONER_LOW,
    ),
    (
        "marker-supply-low",
        ErrorState::LowToner,
        PRINTER_STATE_TONER_LOW,
    ),
    (
        "output-area-full",
        ErrorState::OutputBinFull,
        PRINTER_STATE_OUTPUT_BIN_FULL,
    ),
    (
        "output-area-almost-full",
        ErrorState::OutputBinFull,
        PRINTER_STATE_OUTPUT_BIN_FULL,
    ),
    (
        "spool-area-full",
        ErrorState::Other,
        PRINTER_STATE_OUT_OF_MEMORY,
    ),
    // Reachability problems.
    ("offline", ErrorState::Other, PRINTER_STATE_OFFLINE),
    ("shutdown", ErrorState::Other, PRINTER_STATE_OFFLINE),
    // Operational (no error severity).
    ("paused", ErrorState::NoError, PRINTER_STATE_PAUSED),
    ("moving-to-paused", ErrorState::NoError, PRINTER_STATE_PAUSED),
];

/// Maps the raw `printer-state-reasons` token list (one comma-separated string,
/// taken verbatim from CUPS) into the most-specific [`ErrorState`] plus an
/// OR'd `PRINTER_STATE_*` bitmask suitable for [`crate::PrinterState::from_u32`].
///
/// Returns `(ErrorState::NoError, 0)` when the input is empty, equals `none`,
/// or contains only tokens we don't recognise - matching IPP semantics where
/// the absence of a reason means the printer is healthy.
pub(super) fn map_state_reasons(reasons: &str) -> (ErrorState, u32) {
    let trimmed = reasons.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return (ErrorState::NoError, 0);
    }

    let mut error_state: Option<ErrorState> = None;
    let mut bits: u32 = 0;

    // First pass: OR every recognised reason's bits in. This gives `from_u32`
    // the full picture for its own priority resolution.
    // Second pass (priority): pick the ErrorState of the highest-priority
    // recognised token. We iterate rules in declaration order (most-specific
    // first) and stop at the first hit so the most informative variant wins.
    for raw_token in trimmed.split(',') {
        let token = raw_token.trim().to_ascii_lowercase();
        for (stem, _, mask) in REASON_RULES {
            if token.starts_with(stem) {
                bits |= mask;
                break;
            }
        }
    }
    for (stem, err, _) in REASON_RULES {
        let matched = trimmed
            .split(',')
            .any(|t| t.trim().to_ascii_lowercase().starts_with(stem));
        if matched {
            error_state = Some(err.clone());
            break;
        }
    }

    (error_state.unwrap_or(ErrorState::Other), bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrinterState;

    #[test]
    fn empty_or_none_maps_to_no_error_zero_bits() {
        assert_eq!(map_state_reasons(""), (ErrorState::NoError, 0));
        assert_eq!(map_state_reasons("   "), (ErrorState::NoError, 0));
        assert_eq!(map_state_reasons("none"), (ErrorState::NoError, 0));
        assert_eq!(map_state_reasons("NONE"), (ErrorState::NoError, 0));
    }

    #[test]
    fn unknown_token_maps_to_other_and_zero_bits() {
        let (err, bits) = map_state_reasons("totally-made-up-reason");
        assert_eq!(err, ErrorState::Other);
        assert_eq!(bits, 0);
    }

    #[test]
    fn media_jam_wins_over_co_reported_media_empty() {
        // Priority chain: media-jam is more specific than media-empty, so the
        // ErrorState should be Jammed even though both bits are set.
        let (err, bits) = map_state_reasons("media-empty-warning,media-jam-error");
        assert_eq!(err, ErrorState::Jammed);
        assert!(bits & PRINTER_STATE_PAPER_JAM != 0);
        assert!(bits & PRINTER_STATE_PAPER_OUT != 0);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::PaperJam);
    }

    #[test]
    fn toner_low_maps_to_low_toner() {
        let (err, bits) = map_state_reasons("toner-low-warning");
        assert_eq!(err, ErrorState::LowToner);
        assert_eq!(bits, PRINTER_STATE_TONER_LOW);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::TonerLow);
    }

    #[test]
    fn toner_empty_maps_to_no_toner_with_low_bit_set() {
        let (err, bits) = map_state_reasons("toner-empty-error");
        assert_eq!(err, ErrorState::NoToner);
        assert!(bits & PRINTER_STATE_NO_TONER != 0);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::NoToner);
    }

    #[test]
    fn cover_open_maps_to_door_open() {
        let (err, bits) = map_state_reasons("cover-open-warning");
        assert_eq!(err, ErrorState::DoorOpen);
        assert_eq!(bits, PRINTER_STATE_DOOR_OPEN);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::DoorOpen);
    }

    #[test]
    fn offline_token_sets_offline_bit() {
        let (_, bits) = map_state_reasons("offline-report");
        assert_eq!(bits, PRINTER_STATE_OFFLINE);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::Offline);
    }

    #[test]
    fn paused_is_operational_not_error() {
        let (err, bits) = map_state_reasons("paused");
        assert_eq!(err, ErrorState::NoError);
        assert_eq!(bits, PRINTER_STATE_PAUSED);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::Paused);
    }

    #[test]
    fn output_area_full_maps_to_output_bin_full() {
        let (err, bits) = map_state_reasons("output-area-full-error");
        assert_eq!(err, ErrorState::OutputBinFull);
        assert_eq!(bits, PRINTER_STATE_OUTPUT_BIN_FULL);
        assert_eq!(PrinterState::from_u32(bits), PrinterState::OutputBinFull);
    }

    #[test]
    fn whitespace_around_tokens_is_tolerated() {
        let (err, _) = map_state_reasons("  toner-low  ,  media-low  ");
        // toner-low (LowToner) appears AFTER media-low (LowPaper) in the rule
        // table, so the priority chain picks LowPaper as more specific.
        assert_eq!(err, ErrorState::LowPaper);
    }
}
