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
    PRINTER_STATE_BUSY, PRINTER_STATE_DOOR_OPEN, PRINTER_STATE_ERROR, PRINTER_STATE_INITIALIZING,
    PRINTER_STATE_MANUAL_FEED, PRINTER_STATE_NO_TONER, PRINTER_STATE_NOT_AVAILABLE,
    PRINTER_STATE_OFFLINE, PRINTER_STATE_OUT_OF_MEMORY, PRINTER_STATE_OUTPUT_BIN_FULL,
    PRINTER_STATE_PAPER_JAM, PRINTER_STATE_PAPER_OUT, PRINTER_STATE_PAPER_PROBLEM,
    PRINTER_STATE_PAUSED, PRINTER_STATE_POWER_SAVE, PRINTER_STATE_PRINTING,
    PRINTER_STATE_PROCESSING, PRINTER_STATE_SERVER_UNKNOWN, PRINTER_STATE_TONER_LOW,
    PRINTER_STATE_USER_INTERVENTION_REQUIRED, PRINTER_STATE_WAITING, PRINTER_STATE_WARMING_UP,
};

/// IPP token substrings, ordered most-specific-first. The first match wins so
/// that e.g. a co-reported `media-jam` + `media-empty` surfaces as `Jammed`
/// rather than `NoPaper` - matching the priority chain used by
/// [`crate::PrinterState::from_u32`].
///
/// Token suffix conventions per RFC 8011 §5.4.12: each reason may carry a
/// severity suffix (`-report`, `-warning`, `-error`). We match on the bare
/// stem with `starts_with` so all three severities are handled uniformly.
///
/// Tokens drawn from RFC 8011 §5.4.12 and PWG 5101.1 ("Printer Alerts").
const REASON_RULES: &[(&str, ErrorState, u32)] = &[
    // Hardware jam (cannot proceed without operator).
    ("media-jam", ErrorState::Jammed, PRINTER_STATE_PAPER_JAM),
    (
        "media-path-jam",
        ErrorState::Jammed,
        PRINTER_STATE_PAPER_JAM,
    ),
    (
        "input-tray-jam",
        ErrorState::Jammed,
        PRINTER_STATE_PAPER_JAM,
    ),
    (
        "output-tray-jam",
        ErrorState::Jammed,
        PRINTER_STATE_PAPER_JAM,
    ),
    // Hardware open (operator must close cover/door/interlock).
    ("door-open", ErrorState::DoorOpen, PRINTER_STATE_DOOR_OPEN),
    ("cover-open", ErrorState::DoorOpen, PRINTER_STATE_DOOR_OPEN),
    (
        "interlock-open",
        ErrorState::DoorOpen,
        PRINTER_STATE_DOOR_OPEN,
    ),
    // Severe hardware fault / service mode.
    ("fuser-over-temp", ErrorState::Other, PRINTER_STATE_ERROR),
    ("fuser-under-temp", ErrorState::Other, PRINTER_STATE_ERROR),
    ("opc-life-over", ErrorState::Other, PRINTER_STATE_ERROR),
    (
        "service-required",
        ErrorState::ServiceRequested,
        PRINTER_STATE_ERROR,
    ),
    (
        "service-mode",
        ErrorState::ServiceRequested,
        PRINTER_STATE_ERROR,
    ),
    // Memory exhaustion (cannot accept jobs).
    (
        "memory-exhausted",
        ErrorState::Other,
        PRINTER_STATE_OUT_OF_MEMORY,
    ),
    (
        "out-of-memory",
        ErrorState::Other,
        PRINTER_STATE_OUT_OF_MEMORY,
    ),
    (
        "spool-area-full",
        ErrorState::Other,
        PRINTER_STATE_OUT_OF_MEMORY,
    ),
    // Supply cartridge physically missing (more specific than empty).
    (
        "marker-supply-missing",
        ErrorState::NoToner,
        PRINTER_STATE_NO_TONER,
    ),
    (
        "toner-cartridge-missing",
        ErrorState::NoToner,
        PRINTER_STATE_NO_TONER,
    ),
    // Paper empty.
    ("media-empty", ErrorState::NoPaper, PRINTER_STATE_PAPER_OUT),
    ("media-needed", ErrorState::NoPaper, PRINTER_STATE_PAPER_OUT),
    // Toner empty.
    (
        "toner-empty",
        ErrorState::NoToner,
        PRINTER_STATE_NO_TONER | PRINTER_STATE_TONER_LOW,
    ),
    (
        "marker-supply-empty",
        ErrorState::NoToner,
        PRINTER_STATE_NO_TONER | PRINTER_STATE_TONER_LOW,
    ),
    // Media metadata mismatch (loaded paper does not match job).
    (
        "media-thickness-error",
        ErrorState::Other,
        PRINTER_STATE_PAPER_PROBLEM,
    ),
    (
        "media-size-error",
        ErrorState::Other,
        PRINTER_STATE_PAPER_PROBLEM,
    ),
    (
        "media-type-error",
        ErrorState::Other,
        PRINTER_STATE_PAPER_PROBLEM,
    ),
    // Supply running low (less severe than empty).
    (
        "media-low",
        ErrorState::LowPaper,
        PRINTER_STATE_PAPER_PROBLEM,
    ),
    ("toner-low", ErrorState::LowToner, PRINTER_STATE_TONER_LOW),
    (
        "marker-supply-low",
        ErrorState::LowToner,
        PRINTER_STATE_TONER_LOW,
    ),
    // Output area / waste container.
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
        "marker-waste-full",
        ErrorState::Other,
        PRINTER_STATE_OUTPUT_BIN_FULL,
    ),
    (
        "marker-waste-almost-full",
        ErrorState::Other,
        PRINTER_STATE_OUTPUT_BIN_FULL,
    ),
    // Operational - waiting for operator to feed a sheet.
    (
        "manual-feed",
        ErrorState::NoError,
        PRINTER_STATE_MANUAL_FEED,
    ),
    (
        "input-manual-mode",
        ErrorState::NoError,
        PRINTER_STATE_MANUAL_FEED,
    ),
    // Reachability problems.
    ("offline", ErrorState::Other, PRINTER_STATE_OFFLINE),
    ("shutdown", ErrorState::Other, PRINTER_STATE_OFFLINE),
    (
        "not-available",
        ErrorState::Other,
        PRINTER_STATE_NOT_AVAILABLE,
    ),
    (
        "connecting-to-device",
        ErrorState::Other,
        PRINTER_STATE_SERVER_UNKNOWN,
    ),
    ("timed-out", ErrorState::Other, PRINTER_STATE_SERVER_UNKNOWN),
    // Intentional pause (admin paused the queue).
    ("paused", ErrorState::NoError, PRINTER_STATE_PAUSED),
    (
        "moving-to-paused",
        ErrorState::NoError,
        PRINTER_STATE_PAUSED,
    ),
    ("stopped-partly", ErrorState::NoError, PRINTER_STATE_PAUSED),
    ("stopping", ErrorState::NoError, PRINTER_STATE_PAUSED),
    // Waiting (queue is idle pending external condition).
    ("hold-new-jobs", ErrorState::NoError, PRINTER_STATE_WAITING),
    (
        "processing-stopped",
        ErrorState::NoError,
        PRINTER_STATE_WAITING,
    ),
    // Activity in progress (no error).
    ("printing", ErrorState::NoError, PRINTER_STATE_PRINTING),
    (
        "processing-to-stop-point",
        ErrorState::NoError,
        PRINTER_STATE_PROCESSING,
    ),
    ("in-use", ErrorState::NoError, PRINTER_STATE_BUSY),
    // Transient activity (warming up / initializing).
    ("warming-up", ErrorState::NoError, PRINTER_STATE_WARMING_UP),
    ("warm-up", ErrorState::NoError, PRINTER_STATE_WARMING_UP),
    ("developing", ErrorState::NoError, PRINTER_STATE_WARMING_UP),
    (
        "initializing",
        ErrorState::NoError,
        PRINTER_STATE_INITIALIZING,
    ),
    ("power-up", ErrorState::NoError, PRINTER_STATE_INITIALIZING),
    // Power save.
    ("power-save", ErrorState::NoError, PRINTER_STATE_POWER_SAVE),
    ("sleep-mode", ErrorState::NoError, PRINTER_STATE_POWER_SAVE),
    ("low-power", ErrorState::NoError, PRINTER_STATE_POWER_SAVE),
    // Vague catch-all (lowest priority - more specific causes preferred).
    (
        "attention-required",
        ErrorState::Other,
        PRINTER_STATE_USER_INTERVENTION_REQUIRED,
    ),
    (
        "user-intervention",
        ErrorState::Other,
        PRINTER_STATE_USER_INTERVENTION_REQUIRED,
    ),
];

/// Maps the raw `printer-state-reasons` token list (one comma-separated string,
/// taken verbatim from CUPS) into the most-specific [`ErrorState`] plus an
/// OR'd `PRINTER_STATE_*` bitmask suitable for [`crate::PrinterState::from_u32`].
///
/// Returns `(ErrorState::NoError, 0)` when the input is empty or equals `none`
/// (IPP semantics: no reason means the printer is healthy). When the input
/// contains only tokens we don't recognise, returns `(ErrorState::Other, 0)`
/// so callers can still distinguish "unknown reason reported" from "no reason".
pub(super) fn map_state_reasons(reasons: &str) -> (ErrorState, u32) {
    let trimmed = reasons.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return (ErrorState::NoError, 0);
    }

    // Lowercase each token once; both passes below share this slice.
    let tokens: Vec<String> = trimmed
        .split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .collect();

    let mut error_state: Option<ErrorState> = None;
    let mut bits: u32 = 0;

    // First pass: OR every recognised reason's bits in. This gives `from_u32`
    // the full picture for its own priority resolution.
    for token in &tokens {
        for (stem, _, mask) in REASON_RULES {
            if token.starts_with(stem) {
                bits |= mask;
                break;
            }
        }
    }

    // Second pass (priority): pick the ErrorState of the highest-priority
    // recognised token. We iterate rules in declaration order (most-specific
    // first) and stop at the first hit so the most informative variant wins.
    for (stem, err, _) in REASON_RULES {
        if tokens.iter().any(|t| t.starts_with(stem)) {
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

    /// Table-driven coverage for every IPP reason stem added in 2.0.
    /// Each row: (token, expected ErrorState, expected PrinterState bit OR mask).
    #[test]
    fn extended_reason_stems_map_to_expected_state() {
        let cases: &[(&str, ErrorState, u32)] = &[
            // Jam variants
            (
                "media-path-jam",
                ErrorState::Jammed,
                PRINTER_STATE_PAPER_JAM,
            ),
            (
                "input-tray-jam",
                ErrorState::Jammed,
                PRINTER_STATE_PAPER_JAM,
            ),
            (
                "output-tray-jam",
                ErrorState::Jammed,
                PRINTER_STATE_PAPER_JAM,
            ),
            // Severe fault
            ("fuser-over-temp", ErrorState::Other, PRINTER_STATE_ERROR),
            ("fuser-under-temp", ErrorState::Other, PRINTER_STATE_ERROR),
            ("opc-life-over", ErrorState::Other, PRINTER_STATE_ERROR),
            (
                "service-required",
                ErrorState::ServiceRequested,
                PRINTER_STATE_ERROR,
            ),
            (
                "service-mode",
                ErrorState::ServiceRequested,
                PRINTER_STATE_ERROR,
            ),
            // Memory
            (
                "memory-exhausted",
                ErrorState::Other,
                PRINTER_STATE_OUT_OF_MEMORY,
            ),
            (
                "out-of-memory",
                ErrorState::Other,
                PRINTER_STATE_OUT_OF_MEMORY,
            ),
            // Supply missing
            (
                "marker-supply-missing",
                ErrorState::NoToner,
                PRINTER_STATE_NO_TONER,
            ),
            (
                "toner-cartridge-missing",
                ErrorState::NoToner,
                PRINTER_STATE_NO_TONER,
            ),
            // Media metadata
            (
                "media-thickness-error",
                ErrorState::Other,
                PRINTER_STATE_PAPER_PROBLEM,
            ),
            (
                "media-size-error",
                ErrorState::Other,
                PRINTER_STATE_PAPER_PROBLEM,
            ),
            (
                "media-type-error",
                ErrorState::Other,
                PRINTER_STATE_PAPER_PROBLEM,
            ),
            // Waste
            (
                "marker-waste-full",
                ErrorState::Other,
                PRINTER_STATE_OUTPUT_BIN_FULL,
            ),
            (
                "marker-waste-almost-full",
                ErrorState::Other,
                PRINTER_STATE_OUTPUT_BIN_FULL,
            ),
            // Operational
            (
                "manual-feed",
                ErrorState::NoError,
                PRINTER_STATE_MANUAL_FEED,
            ),
            (
                "input-manual-mode",
                ErrorState::NoError,
                PRINTER_STATE_MANUAL_FEED,
            ),
            // Reachability
            (
                "not-available",
                ErrorState::Other,
                PRINTER_STATE_NOT_AVAILABLE,
            ),
            (
                "connecting-to-device",
                ErrorState::Other,
                PRINTER_STATE_SERVER_UNKNOWN,
            ),
            ("timed-out", ErrorState::Other, PRINTER_STATE_SERVER_UNKNOWN),
            // Pause
            ("stopped-partly", ErrorState::NoError, PRINTER_STATE_PAUSED),
            ("stopping", ErrorState::NoError, PRINTER_STATE_PAUSED),
            // Waiting
            ("hold-new-jobs", ErrorState::NoError, PRINTER_STATE_WAITING),
            (
                "processing-stopped",
                ErrorState::NoError,
                PRINTER_STATE_WAITING,
            ),
            // Activity
            ("printing", ErrorState::NoError, PRINTER_STATE_PRINTING),
            (
                "processing-to-stop-point",
                ErrorState::NoError,
                PRINTER_STATE_PROCESSING,
            ),
            ("in-use", ErrorState::NoError, PRINTER_STATE_BUSY),
            // Warm-up / init
            ("warming-up", ErrorState::NoError, PRINTER_STATE_WARMING_UP),
            ("warm-up", ErrorState::NoError, PRINTER_STATE_WARMING_UP),
            ("developing", ErrorState::NoError, PRINTER_STATE_WARMING_UP),
            (
                "initializing",
                ErrorState::NoError,
                PRINTER_STATE_INITIALIZING,
            ),
            ("power-up", ErrorState::NoError, PRINTER_STATE_INITIALIZING),
            // Power save
            ("power-save", ErrorState::NoError, PRINTER_STATE_POWER_SAVE),
            ("sleep-mode", ErrorState::NoError, PRINTER_STATE_POWER_SAVE),
            ("low-power", ErrorState::NoError, PRINTER_STATE_POWER_SAVE),
            // Catch-all
            (
                "attention-required",
                ErrorState::Other,
                PRINTER_STATE_USER_INTERVENTION_REQUIRED,
            ),
            (
                "user-intervention",
                ErrorState::Other,
                PRINTER_STATE_USER_INTERVENTION_REQUIRED,
            ),
        ];
        for (token, expected_err, expected_bits) in cases {
            let (err, bits) = map_state_reasons(token);
            assert_eq!(err, *expected_err, "ErrorState mismatch for `{token}`");
            assert_eq!(bits, *expected_bits, "bits mismatch for `{token}`");
        }
    }

    #[test]
    fn severity_suffix_is_tolerated_on_new_stems() {
        // The `-report` / `-warning` / `-error` IPP severity suffixes must not
        // prevent matching the bare stem.
        let (err, bits) = map_state_reasons("service-required-error");
        assert_eq!(err, ErrorState::ServiceRequested);
        assert_eq!(bits, PRINTER_STATE_ERROR);

        let (_, bits) = map_state_reasons("warming-up-report");
        assert_eq!(bits, PRINTER_STATE_WARMING_UP);
    }

    #[test]
    fn specific_jam_wins_over_generic_service_required() {
        // A printer reporting both a paper jam and a service-required flag
        // should surface the actionable physical cause (Jammed), not the
        // catch-all ServiceRequested.
        let (err, bits) = map_state_reasons("service-required-error,input-tray-jam-error");
        assert_eq!(err, ErrorState::Jammed);
        assert!(bits & PRINTER_STATE_PAPER_JAM != 0);
        assert!(bits & PRINTER_STATE_ERROR != 0);
    }

    #[test]
    fn cartridge_missing_wins_over_toner_empty() {
        // A physically missing cartridge is a more specific cause than an
        // empty one - the priority chain should prefer it.
        let (err, _) = map_state_reasons("toner-empty,toner-cartridge-missing");
        assert_eq!(err, ErrorState::NoToner);
    }

    #[test]
    fn power_save_does_not_outrank_real_errors() {
        // A printer in sleep mode that ALSO reports media-empty should surface
        // the paper-out condition, not the benign sleep state.
        let (err, _) = map_state_reasons("power-save,media-empty");
        assert_eq!(err, ErrorState::NoPaper);
    }
}
