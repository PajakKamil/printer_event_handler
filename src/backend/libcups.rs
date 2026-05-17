//! libcups2 FFI backend (Linux/macOS).
//!
//! Hand-rolled `extern "C"` bindings to libcups2's destination and job APIs.
//! Used in place of the default `lpstat` subprocess parsing when the
//! `linux-libcups` cargo feature is enabled.
//!
//! All FFI calls are wrapped in [`tokio::task::spawn_blocking`] because
//! libcups is synchronous and can block on network I/O against the CUPS
//! daemon. The unsafe surface is contained inside the [`ffi`] submodule;
//! everything exported up to [`LibCupsBackend`] is safe Rust.
//!
//! Requires `libcups2-dev` (Debian/Ubuntu) / `cups-devel` (Fedora) at build
//! time; cargo expects to find `libcups.so` in the linker search path.

#![cfg(all(unix, feature = "linux-libcups"))]

use crate::printer::state::PrinterState;
use crate::{ErrorState, Job, JobStatus, Printer, PrinterError, PrinterStatus, Result};
use async_trait::async_trait;

use super::PrinterBackend;
use super::lpstat_reasons;

/// IPP `printer-state` values surfaced via the `printer-state` destination
/// option. Defined in RFC 8011 §5.4.11. Named here to avoid magic numbers in
/// the mapping helper.
const IPP_PRINTER_IDLE: i32 = 3;
const IPP_PRINTER_PROCESSING: i32 = 4;
const IPP_PRINTER_STOPPED: i32 = 5;

/// IPP `job-state` values (RFC 8011 §5.3.7) surfaced by `cups_job_t.state`.
const IPP_JSTATE_PENDING: i32 = 3;
const IPP_JSTATE_HELD: i32 = 4;
const IPP_JSTATE_PROCESSING: i32 = 5;
const IPP_JSTATE_STOPPED: i32 = 6;
const IPP_JSTATE_CANCELED: i32 = 7;
const IPP_JSTATE_ABORTED: i32 = 8;
const IPP_JSTATE_COMPLETED: i32 = 9;

/// `which_jobs` selector for `cupsGetJobs2()`. `0` = active jobs only -
/// matches the lpstat backend's behaviour (pending + processing + held).
const CUPS_WHICHJOBS_ACTIVE: i32 = 0;

/// `my_jobs` selector for `cupsGetJobs2()`. `0` = all users' jobs.
const CUPS_MYJOBS_ALL: i32 = 0;

/// Hand-rolled bindings to the subset of libcups2 the backend uses.
/// Everything in here is `unsafe`; safe wrappers live below.
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    /// `cups_option_t` - name/value pair surfaced on `cups_dest_t.options`.
    #[repr(C)]
    pub(super) struct cups_option_t {
        pub name: *mut c_char,
        pub value: *mut c_char,
    }

    /// `cups_dest_t` - a CUPS destination (printer/instance pair).
    #[repr(C)]
    pub(super) struct cups_dest_t {
        pub name: *mut c_char,
        pub instance: *mut c_char,
        pub is_default: c_int,
        pub num_options: c_int,
        pub options: *mut cups_option_t,
    }

    /// `cups_job_t` - one queued or in-flight job. `time_t` is i64 on
    /// 64-bit Linux/macOS; we treat it as `i64` to stay portable.
    #[repr(C)]
    pub(super) struct cups_job_t {
        pub id: c_int,
        pub dest: *mut c_char,
        pub title: *mut c_char,
        pub user: *mut c_char,
        pub format: *mut c_char,
        pub state: c_int,
        pub size: c_int,
        pub priority: c_int,
        pub completed_time: i64,
        pub creation_time: i64,
        pub processing_time: i64,
    }

    #[link(name = "cups")]
    unsafe extern "C" {
        /// Enumerate all destinations on the default server. `http` may be
        /// NULL to use the default connection. Returns the number of
        /// destinations and writes a pointer to the heap-allocated array
        /// into `*dests`.
        pub(super) fn cupsGetDests2(http: *mut c_void, dests: *mut *mut cups_dest_t) -> c_int;

        /// Look up a single destination by name. Returns NULL if no match.
        /// Caller frees with `cupsFreeDests(1, ptr)`.
        pub(super) fn cupsGetNamedDest(
            http: *mut c_void,
            name: *const c_char,
            instance: *const c_char,
        ) -> *mut cups_dest_t;

        /// Free an array returned by `cupsGetDests2`.
        pub(super) fn cupsFreeDests(num_dests: c_int, dests: *mut cups_dest_t);

        /// Enumerate jobs. `name` filters to one destination (NULL = all).
        /// `my_jobs` and `which_jobs` mirror the IPP request attributes.
        pub(super) fn cupsGetJobs2(
            http: *mut c_void,
            jobs: *mut *mut cups_job_t,
            name: *const c_char,
            my_jobs: c_int,
            which_jobs: c_int,
        ) -> c_int;

        /// Free a job array.
        pub(super) fn cupsFreeJobs(num_jobs: c_int, jobs: *mut cups_job_t);

        /// Look up a single option by name from an options array. Returns
        /// NULL if the option is absent.
        pub(super) fn cupsGetOption(
            name: *const c_char,
            num_options: c_int,
            options: *const cups_option_t,
        ) -> *const c_char;
    }
}

/// Safely convert a `*const c_char` returned by libcups into an owned
/// `String`. Returns `None` if the pointer is NULL or the bytes are not
/// valid UTF-8 (libcups returns UTF-8 per RFC 8011 §4.1.4).
unsafe fn cstr_to_string(ptr: *const std::ffi::c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller guarantees the pointer is either NULL or points to a
    // NUL-terminated C string owned by libcups for the duration of the
    // call. We copy the bytes immediately so the lifetime ends here.
    unsafe {
        std::ffi::CStr::from_ptr(ptr)
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

/// Look up an option value by name in a `cups_dest_t.options` array, copying
/// the value into a Rust `String`. Returns `None` when the option is absent.
fn dest_option(dest: &ffi::cups_dest_t, key: &str) -> Option<String> {
    let key_cstr = std::ffi::CString::new(key).ok()?;
    // SAFETY: `dest.options` is a libcups-owned pointer paired with
    // `dest.num_options`. `cupsGetOption` does the bounds-checking. The
    // returned `*const c_char` is owned by libcups; we copy out immediately.
    unsafe {
        let raw = ffi::cupsGetOption(key_cstr.as_ptr(), dest.num_options, dest.options);
        cstr_to_string(raw)
    }
}

/// Map IPP `printer-state` (3 = idle, 4 = processing, 5 = stopped) into
/// [`PrinterStatus`]. Returns `StatusUnknown` for absent or unrecognised
/// values; the caller falls back to `printer-state-reasons` for richer info.
fn map_printer_state(value: Option<&str>) -> PrinterStatus {
    match value.and_then(|s| s.parse::<i32>().ok()) {
        Some(IPP_PRINTER_IDLE) => PrinterStatus::Idle,
        Some(IPP_PRINTER_PROCESSING) => PrinterStatus::Printing,
        Some(IPP_PRINTER_STOPPED) => PrinterStatus::Offline,
        _ => PrinterStatus::StatusUnknown,
    }
}

/// Map an IPP `job-state` integer (returned via `cups_job_t.state`) into the
/// crate's [`JobStatus`] enum. Follows the priority chain documented in
/// `JobStatus::from_u32` (specific cause over generic Error).
fn map_job_state(state: i32) -> JobStatus {
    match state {
        IPP_JSTATE_PENDING => JobStatus::Spooling,
        IPP_JSTATE_HELD => JobStatus::Paused,
        IPP_JSTATE_PROCESSING => JobStatus::Printing,
        IPP_JSTATE_STOPPED => JobStatus::Paused,
        IPP_JSTATE_CANCELED => JobStatus::Deleted,
        IPP_JSTATE_ABORTED => JobStatus::Error,
        IPP_JSTATE_COMPLETED => JobStatus::Complete,
        _ => JobStatus::Unknown,
    }
}

/// Format a `time_t` (seconds since the unix epoch) into the RFC 3339-ish
/// string the rest of the crate uses for `time_submitted_raw`. `0` is
/// libcups's sentinel for "not set" and surfaces as `None`.
fn format_time(seconds: i64) -> Option<String> {
    if seconds <= 0 {
        return None;
    }
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0)?;
    Some(dt.to_rfc3339())
}

/// Convert one libcups destination row into our `Printer`. The destination
/// name is `name` when `instance` is NULL, else `name/instance` (matches the
/// `lpadmin` / `lpoptions` convention).
///
/// All `PrinterError` invariants from CLAUDE.md are preserved: construction
/// routes through `Printer::new_with_state`, which calls `derive_is_offline`
/// and prevents the stored `is_offline` boolean from drifting below typed
/// status / state.
unsafe fn dest_to_printer(dest: &ffi::cups_dest_t) -> Option<Printer> {
    let name_base = unsafe { cstr_to_string(dest.name) }?;
    let instance = unsafe { cstr_to_string(dest.instance) };
    let full_name = match instance.as_deref() {
        Some(inst) if !inst.is_empty() => format!("{name_base}/{inst}"),
        _ => name_base,
    };

    let status = map_printer_state(dest_option(dest, "printer-state").as_deref());
    let reasons = dest_option(dest, "printer-state-reasons").unwrap_or_default();
    let (reason_err, bits) = lpstat_reasons::map_state_reasons(&reasons);

    let derived_state = (bits != 0).then(|| PrinterState::from_u32(bits));
    let final_error = if matches!(reason_err, ErrorState::NoError) {
        ErrorState::NoError
    } else {
        reason_err
    };

    let is_default = dest.is_default != 0 && instance.is_none();

    Some(Printer::new_with_state(
        full_name,
        status,
        derived_state,
        final_error,
        false,
        is_default,
    ))
}

/// Convert one libcups job row into our `Job`. Returns `None` only when
/// `cups_job_t.id` is non-positive (libcups uses 0 as a "no job" sentinel).
unsafe fn job_to_job(job: &ffi::cups_job_t) -> Option<Job> {
    if job.id <= 0 {
        return None;
    }
    let printer_name = unsafe { cstr_to_string(job.dest) };
    let document = unsafe { cstr_to_string(job.title) };
    let owner = unsafe { cstr_to_string(job.user) };
    let submitted = format_time(job.creation_time);

    Some(Job::from_cups(
        job.id as u32,
        printer_name,
        map_job_state(job.state),
        None, // status_string - libcups doesn't surface a free-text status line
        document,
        owner,
        submitted,
        None, // total_pages - cups_job_t doesn't carry page counters
        None, // pages_printed
        None, // name - WMI-only field
    ))
}

/// Pull every destination from the default CUPS server into a `Vec<Printer>`.
/// Blocking - call from inside `spawn_blocking`.
fn enumerate_dests() -> Result<Vec<Printer>> {
    let mut raw: *mut ffi::cups_dest_t = std::ptr::null_mut();
    // SAFETY: `cupsGetDests2` either returns 0 (no destinations - `raw` stays
    // NULL) or a positive count and writes a heap-allocated array into
    // `raw`. We always pair it with `cupsFreeDests` below.
    let count = unsafe { ffi::cupsGetDests2(std::ptr::null_mut(), &mut raw) };
    if count <= 0 || raw.is_null() {
        return Ok(Vec::new());
    }

    let mut printers = Vec::with_capacity(count as usize);
    // SAFETY: `raw[..count]` is the libcups-owned array. We only read from it
    // before calling `cupsFreeDests` below.
    let slice = unsafe { std::slice::from_raw_parts(raw, count as usize) };
    for dest in slice {
        if let Some(printer) = unsafe { dest_to_printer(dest) } {
            printers.push(printer);
        }
    }

    // SAFETY: pairs the `cupsGetDests2` allocation above.
    unsafe { ffi::cupsFreeDests(count, raw) };

    Ok(printers)
}

/// Look up a single destination by name. Returns `Ok(None)` if no printer
/// matches. Blocking - call from inside `spawn_blocking`.
fn fetch_named_dest(name: &str) -> Result<Option<Printer>> {
    let cname = std::ffi::CString::new(name)
        .map_err(|e| PrinterError::Other(format!("invalid name: {e}")))?;
    // SAFETY: `cupsGetNamedDest` returns NULL when no match, else a
    // heap-allocated `cups_dest_t` we must free with `cupsFreeDests(1, ptr)`.
    let ptr =
        unsafe { ffi::cupsGetNamedDest(std::ptr::null_mut(), cname.as_ptr(), std::ptr::null()) };
    if ptr.is_null() {
        return Ok(None);
    }

    // SAFETY: `ptr` points to a valid `cups_dest_t` until we free it.
    let printer = unsafe { dest_to_printer(&*ptr) };
    // SAFETY: pairs `cupsGetNamedDest`. Even when `dest_to_printer` returned
    // `None`, the allocation must still be released.
    unsafe { ffi::cupsFreeDests(1, ptr) };

    Ok(printer)
}

/// Pull jobs from the default CUPS server. `printer_name` filters to one
/// destination (libcups does the filtering when it's `Some`).
/// Blocking - call from inside `spawn_blocking`.
fn enumerate_jobs(printer_name: Option<String>) -> Result<Vec<Job>> {
    let cname_storage = printer_name
        .as_deref()
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|e| PrinterError::Other(format!("invalid name: {e}")))?;
    let cname_ptr = cname_storage
        .as_ref()
        .map_or(std::ptr::null(), |c| c.as_ptr());

    let mut raw: *mut ffi::cups_job_t = std::ptr::null_mut();
    // SAFETY: same contract as `cupsGetDests2`.
    let count = unsafe {
        ffi::cupsGetJobs2(
            std::ptr::null_mut(),
            &mut raw,
            cname_ptr,
            CUPS_MYJOBS_ALL,
            CUPS_WHICHJOBS_ACTIVE,
        )
    };
    if count <= 0 || raw.is_null() {
        return Ok(Vec::new());
    }

    let mut jobs = Vec::with_capacity(count as usize);
    // SAFETY: same contract as for `cups_dest_t` slice above.
    let slice = unsafe { std::slice::from_raw_parts(raw, count as usize) };
    for raw_job in slice {
        if let Some(job) = unsafe { job_to_job(raw_job) } {
            jobs.push(job);
        }
    }

    // SAFETY: pairs `cupsGetJobs2`.
    unsafe { ffi::cupsFreeJobs(count, raw) };

    Ok(jobs)
}

/// libcups2-backed implementation of [`PrinterBackend`]. Drop-in replacement
/// for [`super::LinuxBackend`] when the `linux-libcups` cargo feature is on.
pub struct LibCupsBackend;

#[async_trait]
impl PrinterBackend for LibCupsBackend {
    async fn new() -> Result<Self> {
        use crate::logging::pe_info as info;
        info!("Initializing Linux libcups backend...");
        Ok(Self)
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        tokio::task::spawn_blocking(enumerate_dests)
            .await
            .map_err(|e| PrinterError::Other(format!("libcups join failed: {e}")))?
    }

    async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        let owned = name.to_string();
        tokio::task::spawn_blocking(move || fetch_named_dest(&owned))
            .await
            .map_err(|e| PrinterError::Other(format!("libcups join failed: {e}")))?
    }

    async fn list_jobs(&self, printer_name: Option<&str>) -> Result<Vec<Job>> {
        let owned = printer_name.map(str::to_owned);
        tokio::task::spawn_blocking(move || enumerate_jobs(owned))
            .await
            .map_err(|e| PrinterError::Other(format!("libcups join failed: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_printer_state_recognised_values() {
        assert_eq!(map_printer_state(Some("3")), PrinterStatus::Idle);
        assert_eq!(map_printer_state(Some("4")), PrinterStatus::Printing);
        assert_eq!(map_printer_state(Some("5")), PrinterStatus::Offline);
    }

    #[test]
    fn map_printer_state_unknown_or_missing_is_status_unknown() {
        assert_eq!(map_printer_state(None), PrinterStatus::StatusUnknown);
        assert_eq!(map_printer_state(Some("99")), PrinterStatus::StatusUnknown);
        assert_eq!(
            map_printer_state(Some("not-a-number")),
            PrinterStatus::StatusUnknown
        );
    }

    #[test]
    fn map_job_state_priority_chain() {
        assert_eq!(map_job_state(IPP_JSTATE_PENDING), JobStatus::Spooling);
        assert_eq!(map_job_state(IPP_JSTATE_PROCESSING), JobStatus::Printing);
        assert_eq!(map_job_state(IPP_JSTATE_HELD), JobStatus::Paused);
        assert_eq!(map_job_state(IPP_JSTATE_STOPPED), JobStatus::Paused);
        assert_eq!(map_job_state(IPP_JSTATE_CANCELED), JobStatus::Deleted);
        assert_eq!(map_job_state(IPP_JSTATE_ABORTED), JobStatus::Error);
        assert_eq!(map_job_state(IPP_JSTATE_COMPLETED), JobStatus::Complete);
        assert_eq!(map_job_state(0), JobStatus::Unknown);
    }

    #[test]
    fn format_time_zero_is_none() {
        assert!(format_time(0).is_none());
        assert!(format_time(-1).is_none());
        let s = format_time(1_700_000_000).expect("valid epoch");
        // RFC 3339 prefix matches the year-2023 timestamp.
        assert!(s.starts_with("2023-"));
    }
}
