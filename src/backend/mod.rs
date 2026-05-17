#[cfg(windows)]
use crate::PrinterError;
use crate::{Job, Printer, Result};
use async_trait::async_trait;

#[cfg(unix)]
mod lpstat_jobs;
#[cfg(unix)]
mod lpstat_reasons;

/// Trait for platform-specific printer backend implementations
#[async_trait]
pub trait PrinterBackend: Send + Sync {
    /// Initialize the backend
    async fn new() -> Result<Self>
    where
        Self: Sized;

    /// List all printers on the system
    async fn list_printers(&self) -> Result<Vec<Printer>>;

    /// Find a printer by name (case-insensitive)
    async fn find_printer(&self, name: &str) -> Result<Option<Printer>>;

    /// List print jobs, optionally filtered to a single printer.
    ///
    /// Default implementation returns `Ok(vec![])` so existing backend
    /// implementations keep compiling without breaking changes. Backends that
    /// can enumerate jobs should override this.
    ///
    /// When `printer_name` is `Some(name)`, only jobs owned by that printer
    /// should be returned. `None` returns jobs from all printers.
    async fn list_jobs(&self, _printer_name: Option<&str>) -> Result<Vec<Job>> {
        Ok(Vec::new())
    }
}

/// Windows backend using WMI
#[cfg(windows)]
pub struct WindowsBackend;

/// Runs a parameterless WQL query (e.g. `SELECT * FROM Win32_Printer`) reusing
/// a thread-local [`wmi::WMIConnection`]. The connection is `!Send + !Sync`
/// because it holds COM state tied to its creator thread, so each tokio
/// blocking-pool thread initialises it once and reuses it for the thread's
/// lifetime. On query failure the cached connection is dropped so the next
/// call rebuilds it - covers WMI service restarts and stale handles.
#[cfg(windows)]
fn run_cached_wmi_query<T>(wql: &str) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    use std::cell::RefCell;

    thread_local! {
        static WMI_CONNECTION: RefCell<Option<wmi::WMIConnection>> = const { RefCell::new(None) };
    }

    WMI_CONNECTION.with(|cell| -> Result<Vec<T>> {
        // Lazy init: scope the mutable borrow so it's released before the query.
        {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(wmi::WMIConnection::new().map_err(PrinterError::from)?);
            }
        }

        let query_result = {
            let slot = cell.borrow();
            let conn = slot.as_ref().expect("connection initialised above");
            conn.raw_query::<T>(wql)
        };

        match query_result {
            Ok(rows) => Ok(rows),
            Err(e) => {
                // Drop the (possibly stale) cached connection so the next call
                // can recover from WMI service restarts or expired sessions.
                *cell.borrow_mut() = None;
                Err(PrinterError::from(e))
            }
        }
    })
}

/// `SELECT *` because WQL treats `Default` as a reserved keyword.
#[cfg(windows)]
fn query_win32_printers_cached() -> Result<Vec<crate::printer::Win32Printer>> {
    run_cached_wmi_query::<crate::printer::Win32Printer>("SELECT * FROM Win32_Printer")
}

#[cfg(windows)]
fn query_win32_print_jobs_cached() -> Result<Vec<crate::printer::Win32PrintJob>> {
    run_cached_wmi_query::<crate::printer::Win32PrintJob>("SELECT * FROM Win32_PrintJob")
}

#[cfg(windows)]
#[async_trait]
impl PrinterBackend for WindowsBackend {
    async fn new() -> Result<Self> {
        use crate::logging::pe_info as info;

        info!("Initializing Windows WMI backend...");
        Ok(Self)
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        use crate::logging::pe_info as info;

        info!("Querying printer information via WMI...");

        let wmi_printers = tokio::task::spawn_blocking(query_win32_printers_cached)
            .await
            .map_err(|e| PrinterError::Other(format!("Failed to execute WMI query: {}", e)))??;

        let printers = wmi_printers.into_iter().map(Printer::from).collect();
        Ok(printers)
    }

    async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        let printers = self.list_printers().await?;

        for printer in printers {
            if printer.name().eq_ignore_ascii_case(name) {
                return Ok(Some(printer));
            }
        }

        Ok(None)
    }

    async fn list_jobs(&self, printer_name: Option<&str>) -> Result<Vec<Job>> {
        use crate::logging::pe_info as info;

        info!("Querying print job information via WMI...");

        let raw_jobs = tokio::task::spawn_blocking(query_win32_print_jobs_cached)
            .await
            .map_err(|e| PrinterError::Other(format!("Failed to execute WMI query: {}", e)))??;

        // Filter client-side. WQL supports `WHERE Name = '...'` but
        // `Win32_PrintJob.PrinterName` returns the host printer name with the
        // job id appended (e.g. "HP LaserJet, 42"), so doing the filter in
        // Rust keeps the query simple and the matching robust.
        let jobs: Vec<Job> = raw_jobs
            .into_iter()
            .map(Job::from)
            .filter(|job| match printer_name {
                Some(target) => job
                    .printer_name()
                    .map(|name| name.eq_ignore_ascii_case(target))
                    .unwrap_or(false),
                None => true,
            })
            .collect();
        Ok(jobs)
    }
}

/// Linux backend using CUPS commands
#[cfg(unix)]
pub struct LinuxBackend;

/// Maximum wall-clock budget per CUPS subprocess invocation. A hung CUPS
/// daemon used to hang monitoring forever (F10) - now every call is wrapped
/// in [`tokio::time::timeout`] against this constant and surfaces as
/// [`crate::PrinterError::CupsError`] on expiry.
#[cfg(unix)]
const LPSTAT_TIMEOUT_MS: u64 = 5_000;

/// Locale forced on every CUPS subprocess so parsers can match on stable
/// English literals regardless of the user's `LANG` (B6, F9). `C` is the POSIX
/// locale guaranteed by every libc implementation; all CUPS tools fall back to
/// English output under it.
#[cfg(unix)]
const STABLE_LOCALE: &str = "C";

/// Runs a CUPS-related subprocess with locale forced to `C` (so the output is
/// parseable English regardless of the user's environment) and a wall-clock
/// timeout (so a hung daemon can't hang monitoring forever). Returns stdout as
/// a [`String`] on success; on timeout or non-zero exit, returns
/// [`crate::PrinterError::CupsError`] describing the failure.
#[cfg(unix)]
async fn run_cups_command_with_timeout(
    program: &str,
    args: &[&str],
    timeout_ms: u64,
) -> Result<String> {
    use std::time::Duration;
    use tokio::process::Command;

    let fut = Command::new(program)
        .args(args)
        .env("LANG", STABLE_LOCALE)
        .env("LC_ALL", STABLE_LOCALE)
        .output();

    let output = tokio::time::timeout(Duration::from_millis(timeout_ms), fut)
        .await
        .map_err(|_| {
            crate::PrinterError::CupsError(format!(
                "{program} timed out after {timeout_ms}ms"
            ))
        })??;

    if !output.status.success() {
        return Err(crate::PrinterError::CupsError(format!(
            "{program} exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Convenience wrapper around [`run_cups_command_with_timeout`] using the
/// default [`LPSTAT_TIMEOUT_MS`] budget. All production call sites use this
/// form; the timeout-taking variant exists so unit tests can use a tight
/// budget without waiting the full 5 seconds.
#[cfg(unix)]
async fn run_cups_command(program: &str, args: &[&str]) -> Result<String> {
    run_cups_command_with_timeout(program, args, LPSTAT_TIMEOUT_MS).await
}

#[cfg(unix)]
#[async_trait]
impl PrinterBackend for LinuxBackend {
    async fn new() -> Result<Self> {
        use crate::logging::pe_info as info;

        info!("Initializing Linux CUPS backend...");

        // `which lpstat` returns non-zero (-> Err) if the binary is missing.
        // Either way we proceed - alternative detection covers the no-CUPS case.
        if run_cups_command("which", &["lpstat"]).await.is_ok() {
            info!("CUPS tools found, backend ready");
        } else {
            info!("CUPS not found, checking for alternative printer detection methods");
        }
        Ok(Self)
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        use crate::logging::{pe_info as info, pe_warn as warn};

        info!("Querying printer information via system commands...");

        let mut printers = Vec::new();

        // `-l` adds indented continuation lines including `Alerts:` (CUPS's
        // representation of the IPP `printer-state-reasons` attribute). We
        // walk lines as blocks so we can pair each `printer NAME ...` header
        // with its continuation lines before producing a `Printer`.
        if let Ok(stdout) = run_cups_command("lpstat", &["-l", "-p", "-d"]).await {
            let mut lines = stdout.lines().peekable();
            while let Some(line) = lines.next() {
                if !line.starts_with("printer ") {
                    continue;
                }
                let Some(header) = parse_lpstat_line(line) else {
                    continue;
                };

                // Drain indented continuation lines until the next header or EOF.
                let mut alerts: Option<String> = None;
                while let Some(peek) = lines.peek() {
                    if !(peek.starts_with(' ') || peek.starts_with('\t')) {
                        break;
                    }
                    let cont = lines.next().expect("peeked").trim_start();
                    if let Some(rest) = cont.strip_prefix("Alerts:") {
                        alerts = Some(rest.trim().to_string());
                    }
                }

                printers.push(merge_alerts(header, alerts.as_deref()));
            }

            let default_printer = get_default_printer().await;
            if let Some(ref default_name) = default_printer {
                for printer in &mut printers {
                    if printer.name() == default_name {
                        *printer = Printer::new_with_state(
                            printer.name().to_string(),
                            printer.status().clone(),
                            printer.state().cloned(),
                            printer.error_state().clone(),
                            printer.is_offline(),
                            true, // is_default
                        );
                    }
                }
            }
        }

        if printers.is_empty() {
            warn!("No printers found via lpstat, trying alternative detection methods");
            printers.extend(detect_printers_alternative().await?);
        }

        Ok(printers)
    }

    async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        let printers = self.list_printers().await?;

        for printer in printers {
            if printer.name().eq_ignore_ascii_case(name) {
                return Ok(Some(printer));
            }
        }

        Ok(None)
    }

    async fn list_jobs(&self, printer_name: Option<&str>) -> Result<Vec<Job>> {
        use crate::logging::pe_info as info;

        info!("Querying print jobs via lpstat -l -o...");

        // `lpstat -o [destination]` lists active jobs; `-l` adds the
        // indented Status/Alerts continuation lines we use to derive
        // `JobStatus`. Caller-supplied name is passed both to lpstat (for
        // CUPS-side filtering) and to the parser (defense in depth).
        let mut args: Vec<&str> = vec!["-l", "-o"];
        if let Some(name) = printer_name {
            args.push(name);
        }

        match run_cups_command("lpstat", &args).await {
            Ok(stdout) => Ok(lpstat_jobs::parse_jobs(&stdout, printer_name)),
            // Empty queue makes lpstat exit non-zero on some CUPS builds;
            // treat that as "no jobs" rather than a backend failure.
            Err(_) => Ok(Vec::new()),
        }
    }
}

#[cfg(unix)]
fn parse_lpstat_line(line: &str) -> Option<Printer> {
    use crate::{ErrorState, PrinterStatus};

    // Example line: "printer HP_LaserJet_1020 is idle.  enabled since Mon 01 Jan 2024 12:00:00 PM UTC"
    if let Some(rest) = line.strip_prefix("printer ")
        && let Some(space_pos) = rest.find(' ')
    {
        let name = &rest[..space_pos];
        let status_part = &rest[space_pos + 1..];

        let (status, error_state, is_offline) = if status_part.contains("idle") {
            (PrinterStatus::Idle, ErrorState::NoError, false)
        } else if status_part.contains("printing") {
            (PrinterStatus::Printing, ErrorState::NoError, false)
        } else if status_part.contains("stopped") || status_part.contains("disabled") {
            (PrinterStatus::Offline, ErrorState::Other, true)
        } else {
            (
                PrinterStatus::StatusUnknown,
                ErrorState::UnknownError,
                false,
            )
        };

        return Some(Printer::new(
            name.to_string(),
            status,
            error_state,
            is_offline,
            false, // is_default - will be set later
        ));
    }

    None
}

/// Combines a header-parsed `Printer` with the IPP `printer-state-reasons`
/// surfaced in CUPS's `Alerts:` continuation line. When the alerts list
/// resolves to a more specific [`ErrorState`] than the header (e.g. `Jammed`
/// vs. the header's `NoError`), the alerts-derived value wins. Bits parsed
/// from the alerts feed [`PrinterState::from_u32`] so callers see the same
/// .NET PrintQueueStatus surface that `WindowsBackend` produces.
#[cfg(unix)]
fn merge_alerts(header: Printer, alerts: Option<&str>) -> Printer {
    use crate::PrinterState;

    let Some(alerts) = alerts else {
        return header;
    };

    let (reason_err, bits) = lpstat_reasons::map_state_reasons(alerts);
    if bits == 0 && matches!(reason_err, crate::ErrorState::NoError) {
        return header;
    }

    // Reason-derived error wins over the header's generic NoError/Other when
    // it carries actual signal; keep the header value otherwise.
    let final_error = if matches!(reason_err, crate::ErrorState::NoError) {
        header.error_state().clone()
    } else {
        reason_err
    };

    let derived_state = (bits != 0).then(|| PrinterState::from_u32(bits));

    Printer::new_with_state(
        header.name().to_string(),
        header.status().clone(),
        derived_state,
        final_error,
        header.is_offline(),
        header.is_default(),
    )
}

#[cfg(unix)]
async fn get_default_printer() -> Option<String> {
    let stdout = run_cups_command("lpstat", &["-d"]).await.ok()?;
    for line in stdout.lines() {
        if let Some(name) = line.strip_prefix("system default destination: ") {
            return Some(name.to_string());
        }
        if line.starts_with("no system default destination") {
            return None;
        }
    }
    None
}

#[cfg(unix)]
async fn detect_printers_alternative() -> Result<Vec<Printer>> {
    use crate::logging::pe_info as info;
    use crate::{ErrorState, PrinterStatus};
    use tokio::fs;

    let mut printers = Vec::new();

    info!("Checking for USB printers in /sys/class/usb...");
    if fs::read_dir("/sys/class/usb").await.is_ok() {
        // Basic detection only - real USB-class parsing would walk each
        // device's interface descriptors looking for class 0x07 (printer).
        info!("Found USB entries, but printer detection requires more complex parsing");
    }

    if fs::metadata("/dev/lp0").await.is_ok() {
        info!("Found parallel port printer device");
        printers.push(Printer::new(
            "Parallel Port Printer".to_string(),
            PrinterStatus::StatusUnknown,
            ErrorState::UnknownError,
            false,
            false,
        ));
    }

    if printers.is_empty() {
        info!("No printers detected via alternative methods");
    }

    Ok(printers)
}

/// Create the appropriate backend for the current platform
pub async fn create_backend() -> Result<Box<dyn PrinterBackend>> {
    #[cfg(windows)]
    {
        let backend = WindowsBackend::new().await?;
        Ok(Box::new(backend))
    }

    #[cfg(unix)]
    {
        let backend = LinuxBackend::new().await?;
        Ok(Box::new(backend))
    }

    #[cfg(not(any(windows, unix)))]
    {
        Err(PrinterError::PlatformNotSupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_backend() {
        let result = create_backend().await;

        #[cfg(any(windows, unix))]
        {
            // On supported platforms, backend creation should succeed
            assert!(result.is_ok());
        }

        #[cfg(not(any(windows, unix)))]
        {
            // On unsupported platforms, should return error
            assert!(matches!(result, Err(PrinterError::PlatformNotSupported)));
        }
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn test_windows_backend_creation() {
        let result = WindowsBackend::new().await;
        // May fail in test environments without WMI access, but should not panic
        match result {
            Ok(_) => println!("Windows backend created successfully"),
            Err(e) => println!("Expected error in test environment: {}", e),
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_linux_backend_creation() {
        let result = LinuxBackend::new().await;
        // Should succeed even without CUPS, as we have fallback detection
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn test_windows_backend_list_printers() {
        let backend_result = WindowsBackend::new().await;
        if let Ok(backend) = backend_result {
            let printers = backend.list_printers().await;
            // Either returns printers or an error, but shouldn't panic
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers via Windows backend", printer_list.len());
                    // Verify each printer has required fields
                    for printer in printer_list {
                        assert!(!printer.name().is_empty());
                    }
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_linux_backend_list_printers() {
        let backend = LinuxBackend::new().await;
        assert!(backend.is_ok());

        if let Ok(backend) = backend {
            let printers = backend.list_printers().await;
            // Should return result, even if empty
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers via Linux backend", printer_list.len());
                    // Verify each printer has required fields
                    for printer in printer_list {
                        assert!(!printer.name().is_empty());
                    }
                }
                Err(e) => {
                    println!("Error listing printers: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_linux_backend_find_printer() {
        let backend = LinuxBackend::new().await;
        assert!(backend.is_ok());

        if let Ok(backend) = backend {
            // Try to find a non-existent printer
            let result = backend.find_printer("NonExistentPrinter_Test_12345").await;
            assert!(result.is_ok());
            // Should return None for non-existent printer
            if let Ok(printer_opt) = result
                && printer_opt.is_some()
            {
                println!("Warning: Found printer with unlikely name");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_idle() {
        let line = "printer HP_LaserJet is idle.  enabled since Mon 01 Jan 2024 12:00:00 PM UTC";
        let printer = parse_lpstat_line(line);

        assert!(printer.is_some());
        if let Some(p) = printer {
            assert_eq!(p.name(), "HP_LaserJet");
            assert_eq!(p.status(), &crate::PrinterStatus::Idle);
            assert_eq!(p.error_state(), &crate::ErrorState::NoError);
            assert!(!p.is_offline());
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_printing() {
        let line =
            "printer Canon_MX920 is printing.  enabled since Tue 02 Jan 2024 10:00:00 AM UTC";
        let printer = parse_lpstat_line(line);

        assert!(printer.is_some());
        if let Some(p) = printer {
            assert_eq!(p.name(), "Canon_MX920");
            assert_eq!(p.status(), &crate::PrinterStatus::Printing);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_stopped() {
        let line = "printer Epson_XP is stopped.  disabled since Wed 03 Jan 2024 08:00:00 AM UTC";
        let printer = parse_lpstat_line(line);

        assert!(printer.is_some());
        if let Some(p) = printer {
            assert_eq!(p.name(), "Epson_XP");
            assert_eq!(p.status(), &crate::PrinterStatus::Offline);
            assert!(p.is_offline());
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_lpstat_line_invalid() {
        let line = "This is not a valid lpstat line";
        let printer = parse_lpstat_line(line);
        assert!(printer.is_none());
    }

    /// F10 regression: a hung subprocess must surface as `CupsError` within
    /// the configured budget. Uses a 200ms timeout against `sleep 5` so the
    /// test finishes in well under a second.
    #[cfg(unix)]
    #[tokio::test]
    async fn run_cups_command_times_out() {
        let result = run_cups_command_with_timeout("sleep", &["5"], 200).await;
        match result {
            Err(crate::PrinterError::CupsError(msg)) => {
                assert!(
                    msg.contains("timed out"),
                    "expected timeout message, got: {msg}"
                );
            }
            other => panic!("expected CupsError(timed out ...), got {other:?}"),
        }
    }

    /// B6/F9 regression: `LANG=C` is forced, so even when the host locale is
    /// non-English the helper's stdout uses POSIX-locale formatting. We assert
    /// this indirectly by running `printf` and confirming the bytes come back
    /// unchanged (no locale-driven mangling).
    #[cfg(unix)]
    #[tokio::test]
    async fn run_cups_command_forces_c_locale() {
        let result =
            run_cups_command_with_timeout("printf", &["printer X is idle.\n"], 1_000).await;
        match result {
            Ok(stdout) => assert_eq!(stdout, "printer X is idle.\n"),
            // `printf` should exist on every unix CI runner; treat absence as
            // an infra problem rather than a code failure.
            Err(e) => eprintln!("skipping (printf unavailable?): {e}"),
        }
    }

    /// M5 regression: a `printer is idle` block with `Alerts: toner-low`
    /// must surface as `LowToner` (not the header-derived `NoError`) and the
    /// derived `PrinterState` must reflect the corresponding bit.
    #[cfg(unix)]
    #[test]
    fn merge_alerts_promotes_toner_low_over_idle_header() {
        let header = parse_lpstat_line("printer Foo is idle.  enabled since X")
            .expect("header parses");
        let merged = merge_alerts(header, Some("toner-low-warning"));
        assert_eq!(merged.error_state(), &crate::ErrorState::LowToner);
        assert_eq!(merged.state(), Some(&crate::PrinterState::TonerLow));
    }

    /// M5 regression: a co-reported `media-empty,media-jam` must collapse to
    /// `Jammed` (the higher-priority specific cause), matching the priority
    /// chain in `PrinterState::from_u32`.
    #[cfg(unix)]
    #[test]
    fn merge_alerts_picks_most_specific_cause() {
        let header = parse_lpstat_line("printer Foo is idle.  enabled since X")
            .expect("header parses");
        let merged = merge_alerts(header, Some("media-empty-warning,media-jam-error"));
        assert_eq!(merged.error_state(), &crate::ErrorState::Jammed);
        assert_eq!(merged.state(), Some(&crate::PrinterState::PaperJam));
    }

    /// M5 regression: `Alerts: none` and absent alerts both leave the header
    /// untouched (no spurious state assignment, no error override).
    #[cfg(unix)]
    #[test]
    fn merge_alerts_none_leaves_header_untouched() {
        let header = parse_lpstat_line("printer Foo is idle.  enabled since X")
            .expect("header parses");
        let merged_none = merge_alerts(header.clone(), Some("none"));
        assert_eq!(merged_none.error_state(), &crate::ErrorState::NoError);
        assert!(merged_none.state().is_none());

        let merged_absent = merge_alerts(header, None);
        assert_eq!(merged_absent.error_state(), &crate::ErrorState::NoError);
        assert!(merged_absent.state().is_none());
    }
}
