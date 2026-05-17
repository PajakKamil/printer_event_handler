//! `lpstat -l -o` parser for the Linux/CUPS backend.
//!
//! CUPS exposes the print queue via `lpstat -o`, which lists active jobs in a
//! per-line format like:
//!
//! ```text
//! HP_LaserJet_1020-123    user      45056 bytes   Mon 01 Jan 2026 12:00:00 PM UTC
//! ```
//!
//! With the `-l` (long) flag, additional indented continuation lines surface
//! the per-job state under `Status:` and the IPP `job-state-reasons` under
//! `Alerts:`. This module walks the output block-aware (header + continuation
//! lines) and produces [`Job`]s with the subset of fields CUPS makes
//! observable. Fields CUPS doesn't expose (page counters, `job_status_code`)
//! stay `None`.

#![cfg(unix)]

use crate::{Job, JobStatus};

/// Parses `lpstat -l -o [printer_name]` stdout into [`Job`]s.
///
/// `name_filter` is the printer name supplied by the caller (already passed to
/// lpstat) - we re-apply it client-side as a defense in depth, in case lpstat
/// returns more than requested (some CUPS versions ignore the filter for jobs
/// owned by a renamed-but-still-spooled printer).
pub(super) fn parse_jobs(stdout: &str, name_filter: Option<&str>) -> Vec<Job> {
    let mut jobs = Vec::new();
    let mut lines = stdout.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        // Skip blank lines and continuation lines that don't belong to a
        // job we picked up.
        if trimmed.is_empty() || line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }

        let Some(JobHeaderFields {
            job_id,
            printer_name,
            owner,
            submitted_time_raw,
        }) = parse_header(line)
        else {
            continue;
        };

        // Read continuation lines: anything starting with whitespace until
        // the next non-indented line or EOF.
        let mut status_string: Option<String> = None;
        let mut alerts: Option<String> = None;
        while let Some(peek) = lines.peek() {
            if !(peek.starts_with(' ') || peek.starts_with('\t')) {
                break;
            }
            let cont = lines.next().expect("peeked").trim_start();
            if let Some(rest) = cont.strip_prefix("Status:") {
                status_string = Some(rest.trim().to_string());
            } else if let Some(rest) = cont.strip_prefix("Alerts:") {
                alerts = Some(rest.trim().to_string());
            }
        }

        if let Some(want) = name_filter
            && printer_name.as_deref().map(|p| !p.eq_ignore_ascii_case(want)).unwrap_or(true)
        {
            continue;
        }

        let status = map_job_state(status_string.as_deref(), alerts.as_deref());

        jobs.push(Job::from_lpstat(
            job_id,
            printer_name,
            status,
            status_string,
            owner,
            submitted_time_raw,
        ));
    }

    jobs
}

/// Parsed fields from a job header line. Returned by [`parse_header`] and
/// destructured by the main loop; named so clippy doesn't flag the otherwise
/// quad-`Option<String>` tuple as `clippy::type_complexity`.
struct JobHeaderFields {
    job_id: u32,
    printer_name: Option<String>,
    owner: Option<String>,
    submitted_time_raw: Option<String>,
}

/// Header parse: extracts the fields from one `lpstat -o` header line.
/// Returns `None` when the line doesn't look like a job header (no
/// `<printer>-<digits>` first token).
fn parse_header(line: &str) -> Option<JobHeaderFields> {
    let mut iter = line.split_whitespace();
    let first = iter.next()?;
    let (printer_name, job_id) = split_printer_and_job(first)?;

    let owner = iter.next().map(str::to_string);
    let remainder: Vec<&str> = iter.collect();

    // CUPS prints the size as e.g. `45056 bytes` then the submission date.
    // We don't surface size on `Job`, so just look for the date which starts
    // at the first token that isn't a size suffix.
    let time_raw = if remainder.is_empty() {
        None
    } else {
        // Strip leading numeric size + optional `bytes` unit when present.
        let mut start = 0usize;
        if remainder
            .first()
            .map(|t| t.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
        {
            start += 1;
            if remainder
                .get(start)
                .map(|t| *t == "bytes")
                .unwrap_or(false)
            {
                start += 1;
            }
        }
        if start >= remainder.len() {
            None
        } else {
            Some(remainder[start..].join(" "))
        }
    };

    Some(JobHeaderFields {
        job_id,
        printer_name: Some(printer_name),
        owner,
        submitted_time_raw: time_raw,
    })
}

/// Splits a CUPS job identifier like `HP_LaserJet_1020-123` into the printer
/// name (`HP_LaserJet_1020`) and the numeric job ID (`123`). Returns `None`
/// when the trailing segment after the last `-` isn't numeric.
fn split_printer_and_job(token: &str) -> Option<(String, u32)> {
    let (printer, id_str) = token.rsplit_once('-')?;
    let job_id: u32 = id_str.parse().ok()?;
    Some((printer.to_string(), job_id))
}

/// Maps the CUPS `Status:` line and `Alerts:` IPP job-state-reasons to the
/// most informative [`JobStatus`]. Priority chain: explicit `Status:` text
/// wins, falling back to alert reasons, then `Unknown`.
fn map_job_state(status: Option<&str>, alerts: Option<&str>) -> JobStatus {
    if let Some(s) = status {
        let lower = s.to_ascii_lowercase();
        if lower.contains("processing") || lower.contains("in progress") {
            return JobStatus::Printing;
        }
        if lower.contains("pending") {
            return JobStatus::Spooling;
        }
        if lower.contains("held") {
            return JobStatus::Paused;
        }
        if lower.contains("canceled") || lower.contains("cancelled") || lower.contains("aborted") {
            return JobStatus::Deleted;
        }
        if lower.contains("stopped") {
            return JobStatus::Error;
        }
        if lower.contains("completed") {
            return JobStatus::Complete;
        }
    }

    if let Some(a) = alerts {
        for raw_token in a.split(',') {
            let token = raw_token.trim().to_ascii_lowercase();
            if token.starts_with("job-printing") {
                return JobStatus::Printing;
            }
            if token.starts_with("job-incoming") || token.starts_with("job-pending") {
                return JobStatus::Spooling;
            }
            if token.starts_with("job-hold") {
                return JobStatus::Paused;
            }
            if token.starts_with("job-canceled") || token.starts_with("job-aborted") {
                return JobStatus::Deleted;
            }
            if token.starts_with("job-completed") {
                return JobStatus::Complete;
            }
            if token.starts_with("job-stopped") {
                return JobStatus::Error;
            }
        }
    }

    JobStatus::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = "\
HP_LaserJet_1020-123    user      45056 bytes   Mon 01 Jan 2026 12:00:00 PM UTC
        Status: processing
        Alerts: job-printing
HP_LaserJet_1020-124    alice     12000 bytes   Mon 01 Jan 2026 12:01:00 PM UTC
        Status: pending
        Alerts: job-incoming
Canon_MX920-77    bob     5000 bytes   Mon 01 Jan 2026 12:02:00 PM UTC
        Status: held by user
";

    #[test]
    fn parses_three_jobs_with_correct_ids_and_owners() {
        let jobs = parse_jobs(SAMPLE_OUTPUT, None);
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].job_id(), 123);
        assert_eq!(jobs[0].printer_name(), Some("HP_LaserJet_1020"));
        assert_eq!(jobs[0].owner(), Some("user"));
        assert_eq!(jobs[0].status(), &JobStatus::Printing);

        assert_eq!(jobs[1].job_id(), 124);
        assert_eq!(jobs[1].owner(), Some("alice"));
        assert_eq!(jobs[1].status(), &JobStatus::Spooling);

        assert_eq!(jobs[2].job_id(), 77);
        assert_eq!(jobs[2].printer_name(), Some("Canon_MX920"));
        assert_eq!(jobs[2].status(), &JobStatus::Paused);
    }

    #[test]
    fn name_filter_excludes_jobs_from_other_printers() {
        let jobs = parse_jobs(SAMPLE_OUTPUT, Some("HP_LaserJet_1020"));
        assert_eq!(jobs.len(), 2);
        assert!(
            jobs.iter()
                .all(|j| j.printer_name() == Some("HP_LaserJet_1020"))
        );
    }

    #[test]
    fn name_filter_is_case_insensitive() {
        let jobs = parse_jobs(SAMPLE_OUTPUT, Some("hp_laserjet_1020"));
        assert_eq!(jobs.len(), 2);
    }

    #[test]
    fn job_state_reason_falls_back_to_alerts_when_status_absent() {
        let stdout = "HP_LaserJet_1020-200    user     1000 bytes   Mon 01 Jan 2026\n        Alerts: job-completed-successfully\n";
        let jobs = parse_jobs(stdout, None);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status(), &JobStatus::Complete);
    }

    #[test]
    fn malformed_header_skipped_silently() {
        let stdout = "This is not a job header\nHP_LaserJet_1020-999    user     0 bytes   X\n";
        let jobs = parse_jobs(stdout, None);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id(), 999);
    }

    #[test]
    fn empty_input_yields_empty_vec() {
        assert!(parse_jobs("", None).is_empty());
    }

    #[test]
    fn time_submitted_is_captured() {
        let jobs = parse_jobs(SAMPLE_OUTPUT, None);
        assert_eq!(
            jobs[0].time_submitted_raw(),
            Some("Mon 01 Jan 2026 12:00:00 PM UTC")
        );
    }

    #[test]
    fn status_string_preserved() {
        let jobs = parse_jobs(SAMPLE_OUTPUT, None);
        assert_eq!(jobs[0].status_string(), Some("processing"));
        assert_eq!(jobs[2].status_string(), Some("held by user"));
    }
}
