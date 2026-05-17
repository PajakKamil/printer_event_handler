//! End-to-end integration tests covering the public `PrinterMonitor` API
//! and the printer events the library surfaces.
//!
//! The suite mixes two modes in one file:
//!
//! * **Real-OS tests** create a real printer on the host system,
//!   trigger genuine events (disappearance, paper-out injection,
//!   pause/resume, default flip, rename, job submission, ...) and
//!   assert the library's callbacks fire with the expected
//!   [`PropertyChange`]s. These tests skip with a `[SKIP]` message when
//!   the host can't grant the required privileges (e.g. `Add-Printer`
//!   on Windows, `lpadmin` on Linux), so the suite stays un-`#[ignore]`
//!   without turning into a privilege check.
//!
//! * **Scripted-backend tests** swap in a `ScriptedBackend` via
//!   [`PrinterMonitor::from_backend`] and drive `monitor_printer_changes`
//!   with a hand-crafted sequence of `find_printer` responses. These
//!   cover every `PropertyChange` variant, the disappearance/reappearance
//!   `PresenceTracker` invariant, the
//!   `MAX_CONSECUTIVE_MONITOR_ERRORS` transient-error tolerance, and the
//!   per-`PrinterError` propagation paths - none of which can be
//!   exercised reliably through a real spooler/CUPS.
//!
//! ## Privileges
//!
//! - **Windows**: the account must be able to run `Add-Printer` (any
//!   user with the Print Operator or local admin role; standard
//!   accounts on a typical workstation install qualify). State
//!   injection additionally uses `Add-Type` + P/Invoke against
//!   `winspool.drv`; this is allowed for any process that already owns
//!   the test printer (`Add-Printer` grants Manage Printer rights to
//!   the creating account).
//! - **Linux**: root or membership in `lpadmin`. CUPS must be running.
//!
//! ## Linux scope limit
//!
//! `LinuxBackend` currently only parses `idle / printing / stopped`
//! from `lpstat -p`. State-injection-style coverage (paper-out, jam,
//! door-open, toner) on Linux is therefore deferred to the M5
//! enhancement that parses `printer-state-reasons`; until then,
//! per-event Linux assertions are limited to disappear/reappear,
//! offline/idle, default-printer flips, and best-effort printing.
//!
//! ## Concurrency
//!
//! `cargo test` runs integration-test functions in parallel by
//! default. Each real-OS test in this file uses a uniquely-named
//! printer (`RustPrinterEventHandlerTest_42_*`) so the per-test
//! `PrinterGuard` cleanup never races a sibling test's monitor. The
//! three original tests still share `TEST_PRINTER`; that's tolerated
//! for back-compat - new real-OS tests should pick a fresh name.

use async_trait::async_trait;
use printer_event_handler::backend::PrinterBackend;
use printer_event_handler::{
    CancellationToken, ErrorState, MonitorableProperty, Printer, PrinterChanges, PrinterError,
    PrinterMonitor, PrinterState, PrinterStatus, PropertyChange, Result,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio::time::timeout;

// ---------- Printer names ----------

/// Test printer name. Must not collide with any real printer on the
/// host system - the suffix is deliberately unusual.
const TEST_PRINTER: &str = "RustPrinterEventHandlerTest_42";
const TEST_PRINTER_A: &str = "RustPrinterEventHandlerTest_42_A";
const TEST_PRINTER_B: &str = "RustPrinterEventHandlerTest_42_B";

const TEST_PRINTER_CASEI: &str = "RustPrinterEventHandlerTest_42_CaseI";
const TEST_PRINTER_LIST_AFTER_REMOVE: &str = "RustPrinterEventHandlerTest_42_ListAfterRemove";
const TEST_PRINTER_SUMMARY: &str = "RustPrinterEventHandlerTest_42_Summary";
const TEST_PRINTER_ACCESSORS: &str = "RustPrinterEventHandlerTest_42_Accessors";
const TEST_PRINTER_INITIAL_SNAPSHOT: &str = "RustPrinterEventHandlerTest_42_InitialSnapshot";
const TEST_PRINTER_CANCEL: &str = "RustPrinterEventHandlerTest_42_Cancel";

const TEST_PRINTER_PAPER_OUT: &str = "RustPrinterEventHandlerTest_42_PaperOut";
const TEST_PRINTER_DOOR_OPEN: &str = "RustPrinterEventHandlerTest_42_DoorOpen";
const TEST_PRINTER_PAPER_JAM: &str = "RustPrinterEventHandlerTest_42_PaperJam";
const TEST_PRINTER_TONER_LOW: &str = "RustPrinterEventHandlerTest_42_TonerLow";
const TEST_PRINTER_NO_TONER: &str = "RustPrinterEventHandlerTest_42_NoToner";
const TEST_PRINTER_OUTPUT_BIN: &str = "RustPrinterEventHandlerTest_42_OutputBin";
const TEST_PRINTER_OOM: &str = "RustPrinterEventHandlerTest_42_Oom";
const TEST_PRINTER_USER_INT: &str = "RustPrinterEventHandlerTest_42_UserInt";
const TEST_PRINTER_ERROR: &str = "RustPrinterEventHandlerTest_42_Error";
const TEST_PRINTER_COMBINED: &str = "RustPrinterEventHandlerTest_42_Combined";
const TEST_PRINTER_PAUSE_RESUME: &str = "RustPrinterEventHandlerTest_42_PauseResume";
const TEST_PRINTER_SET_DEFAULT: &str = "RustPrinterEventHandlerTest_42_SetDefault";
const TEST_PRINTER_RENAME_SRC: &str = "RustPrinterEventHandlerTest_42_RenameSrc";
const TEST_PRINTER_RENAME_DST: &str = "RustPrinterEventHandlerTest_42_RenameDst";
const TEST_PRINTER_SUBMIT_JOB: &str = "RustPrinterEventHandlerTest_42_SubmitJob";

#[cfg(unix)]
const TEST_PRINTER_CUPS_DISABLE: &str = "RustPrinterEventHandlerTest_42_CupsDisable";
#[cfg(unix)]
const TEST_PRINTER_CUPS_ENABLE: &str = "RustPrinterEventHandlerTest_42_CupsEnable";
#[cfg(unix)]
const TEST_PRINTER_LINUX_DEFAULT: &str = "RustPrinterEventHandlerTest_42_LinuxDefault";
#[cfg(unix)]
const TEST_PRINTER_LINUX_SUBMIT: &str = "RustPrinterEventHandlerTest_42_LinuxSubmit";

// ---------- Timing ----------

/// Poll interval for real-OS monitors. Tight enough that
/// disappearance/reappearance events surface within a few seconds.
const MONITOR_INTERVAL_MS: u64 = 500;
/// Poll interval for scripted tests - small so the suite stays fast.
const SCRIPTED_INTERVAL_MS: u64 = 50;
/// Upper bound on how long we wait for an event to appear after we
/// trigger it. WMI/CUPS aren't instant; this is intentionally generous.
const EVENT_DEADLINE: Duration = Duration::from_secs(15);
/// Shorter deadline for scripted tests since they don't hit a real
/// spooler.
const SCRIPTED_EVENT_DEADLINE: Duration = Duration::from_secs(3);
/// How long to sleep after spawning the monitor before triggering the
/// first event - lets the monitor record the baseline (which is
/// silent by design).
const INITIAL_CAPTURE_DELAY: Duration = Duration::from_millis(1_500);
/// Same idea, but for scripted monitors at the much tighter scripted
/// interval.
const SCRIPTED_INITIAL_CAPTURE_DELAY: Duration = Duration::from_millis(120);
/// Max wait for the monitor task to honour cancellation before we
/// give up on a clean shutdown.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// Cadence for `poll_until` - small enough to not blunt EVENT_DEADLINE.
const POLL_TICK: Duration = Duration::from_millis(100);

/// Mirror of the library's `MAX_CONSECUTIVE_MONITOR_ERRORS` constant
/// (`src/monitor/mod.rs`). Re-declared here because the lib's symbol is
/// `pub(super)`. If the lib constant changes, the
/// `scripted_fatal_after_max_consecutive_errors` test will fail loudly
/// at the assertion stage - that's the early-warning signal to update
/// this value.
const MAX_CONSECUTIVE_MONITOR_ERRORS: u32 = 5;

// ---------- Win32 PRINTER_STATUS_* bit values ----------
// Same numeric values the Win32 `SetPrinter` Level-6 call interprets,
// and what `Win32_Printer.PrinterState` reflects back through WMI for
// the library to parse as `PrinterState`. Local copies of the
// `PRINTER_STATE_*` constants in `src/printer/state.rs` so that
// module's `pub(super)` visibility doesn't need to be widened.

#[cfg(windows)]
const PRINTER_STATUS_ERROR: u32 = 0x0000_0002;
#[cfg(windows)]
const PRINTER_STATUS_PAPER_JAM: u32 = 0x0000_0008;
#[cfg(windows)]
const PRINTER_STATUS_PAPER_OUT: u32 = 0x0000_0010;
#[cfg(windows)]
const PRINTER_STATUS_OUTPUT_BIN_FULL: u32 = 0x0000_0800;
#[cfg(windows)]
const PRINTER_STATUS_TONER_LOW: u32 = 0x0002_0000;
#[cfg(windows)]
const PRINTER_STATUS_NO_TONER: u32 = 0x0004_0000;
#[cfg(windows)]
const PRINTER_STATUS_USER_INTERVENTION: u32 = 0x0010_0000;
#[cfg(windows)]
const PRINTER_STATUS_OUT_OF_MEMORY: u32 = 0x0020_0000;
#[cfg(windows)]
const PRINTER_STATUS_DOOR_OPEN: u32 = 0x0040_0000;

// ============================================================================
// Platform helpers
// ============================================================================

#[cfg(windows)]
mod platform {
    use std::process::Command;

    fn powershell(script: &str) -> bool {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn powershell_capture(script: &str) -> Option<String> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Returns `true` if the printer was created. Probes for an
    /// installed driver from a small candidate list (Microsoft Print
    /// to PDF and XPS ship on every modern Windows; Generic / Text
    /// Only covers older SKUs and some Server installs). Stderr is
    /// suppressed - failure is signalled via the exit code so callers
    /// can `skip()` cleanly without spraying red text across the test
    /// output when a driver simply isn't installed.
    pub fn create_printer(name: &str) -> bool {
        // The `nul:` port might already be present from a prior run; we
        // try-add silently so a duplicate doesn't poison the run.
        powershell("Add-PrinterPort -Name 'nul:' -ErrorAction SilentlyContinue 2>$null");
        let script = format!(
            r#"
$ErrorActionPreference = 'SilentlyContinue'
$candidates = @('Microsoft Print to PDF', 'Microsoft XPS Document Writer', 'Generic / Text Only')
$driver = $candidates | Where-Object {{ Get-PrinterDriver -Name $_ 2>$null }} | Select-Object -First 1
if (-not $driver) {{ exit 1 }}
Add-Printer -Name '{name}' -DriverName $driver -PortName 'nul:' 2>$null
if (-not $?) {{ exit 1 }}
exit 0
"#,
            name = name
        );
        powershell(&script)
    }

    pub fn remove_printer(name: &str) {
        let _ = powershell(&format!(
            "Remove-Printer -Name '{}' -ErrorAction SilentlyContinue 2>$null",
            name
        ));
    }

    /// Sets the printer's `PRINTER_INFO_6.dwStatus` bitmask via the
    /// Win32 `SetPrinter` API. This is the documented way to drive the
    /// `Win32_Printer.PrinterState` field that the library's
    /// `PrinterState::from_u32` parses - `Set-CimInstance` on the same
    /// property is a no-op because the WMI provider treats it as
    /// read-only.
    ///
    /// Returns `false` if the P/Invoke fails (typical reasons: the
    /// account doesn't own the printer, or some Server SKUs lock down
    /// `winspool.drv`). Tests should treat `false` as a soft skip.
    pub fn inject_state(name: &str, status_flags: u32) -> bool {
        // Fully-qualified C# names so we don't need `-UsingNamespace`:
        // on Polish-locale PowerShell (and probably others), passing
        // `-UsingNamespace System.Runtime.InteropServices` plus the
        // compiler's own auto-emitted using causes a duplicate-using
        // warning that's treated as an error (-warnaserror).
        let script = format!(
            r#"
$ErrorActionPreference = 'Stop'
Add-Type -Namespace Win32 -Name PrintInjection -MemberDefinition @'
    [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
    public struct PRINTER_INFO_6 {{ public uint Status; }}
    [System.Runtime.InteropServices.DllImport("winspool.drv", SetLastError=true, CharSet=System.Runtime.InteropServices.CharSet.Auto)]
    public static extern bool OpenPrinter(string n, out System.IntPtr h, System.IntPtr d);
    [System.Runtime.InteropServices.DllImport("winspool.drv", SetLastError=true)]
    public static extern bool SetPrinter(System.IntPtr h, uint lvl, ref PRINTER_INFO_6 i, uint cmd);
    [System.Runtime.InteropServices.DllImport("winspool.drv")]
    public static extern bool ClosePrinter(System.IntPtr h);
'@ 2>$null
$h = [System.IntPtr]::Zero
if (-not [Win32.PrintInjection]::OpenPrinter('{name}', [ref]$h, [System.IntPtr]::Zero)) {{ exit 1 }}
$i = New-Object Win32.PrintInjection+PRINTER_INFO_6
$i.Status = {flags}
$ok = [Win32.PrintInjection]::SetPrinter($h, 6, [ref]$i, 0)
[Win32.PrintInjection]::ClosePrinter($h) | Out-Null
if (-not $ok) {{ exit 1 }}
"#,
            name = name,
            flags = status_flags
        );
        powershell(&script)
    }

    pub fn clear_state(name: &str) -> bool {
        inject_state(name, 0)
    }

    /// Pause/Resume go through the Win32_Printer WMI methods because
    /// the corresponding cmdlets (`Suspend-Printer` / `Resume-Printer`)
    /// aren't present on every Windows install - they live in the
    /// PrintManagement module which is optional on some SKUs. The WMI
    /// methods have shipped since Windows 7.
    pub fn pause_printer(name: &str) -> bool {
        powershell(&format!(
            "(Get-CimInstance -Class Win32_Printer -Filter \"Name='{}'\") | Invoke-CimMethod -MethodName Pause -ErrorAction SilentlyContinue | Out-Null",
            name
        ))
    }

    pub fn resume_printer(name: &str) -> bool {
        powershell(&format!(
            "(Get-CimInstance -Class Win32_Printer -Filter \"Name='{}'\") | Invoke-CimMethod -MethodName Resume -ErrorAction SilentlyContinue | Out-Null",
            name
        ))
    }

    pub fn set_default(name: &str) -> bool {
        powershell(&format!(
            "(Get-CimInstance -Class Win32_Printer -Filter \"Name='{}'\") | Invoke-CimMethod -MethodName SetDefaultPrinter | Out-Null",
            name
        ))
    }

    pub fn get_default() -> Option<String> {
        powershell_capture("(Get-CimInstance -Class Win32_Printer -Filter 'Default=True').Name")
            .filter(|s| !s.is_empty())
    }

    pub fn rename_printer(old: &str, new: &str) -> bool {
        powershell(&format!(
            "Rename-Printer -Name '{}' -NewName '{}' -ErrorAction SilentlyContinue",
            old, new
        ))
    }

    pub fn submit_job(name: &str) -> bool {
        powershell(&format!(
            "'integration-test-payload' | Out-Printer -Name '{}' -ErrorAction SilentlyContinue",
            name
        ))
    }
}

#[cfg(unix)]
mod platform {
    use std::process::Command;

    pub fn create_printer(name: &str) -> bool {
        Command::new("lpadmin")
            .args(["-p", name, "-E", "-v", "file:///dev/null"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn remove_printer(name: &str) {
        let _ = Command::new("lpadmin").args(["-x", name]).status();
    }

    /// Linux state injection isn't available - CUPS doesn't expose a
    /// programmatic way to set `printer-state-reasons` from userspace
    /// without first parsing them in the backend. Returns `false` so
    /// every Windows injection test skips cleanly on Linux. The
    /// follow-up M5 enhancement to `LinuxBackend` is what unlocks
    /// this.
    pub fn inject_state(_name: &str, _flags: u32) -> bool {
        false
    }

    pub fn clear_state(_name: &str) -> bool {
        false
    }

    pub fn cups_disable(name: &str) -> bool {
        Command::new("cupsdisable")
            .arg(name)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn cups_enable(name: &str) -> bool {
        Command::new("cupsenable")
            .arg(name)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn set_default(name: &str) -> bool {
        Command::new("lpadmin")
            .args(["-d", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn get_default() -> Option<String> {
        let output = Command::new("lpstat").arg("-d").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(name) = line.strip_prefix("system default destination: ") {
                return Some(name.trim().to_string());
            }
        }
        None
    }

    pub fn submit_job(name: &str) -> bool {
        Command::new("lp")
            .args(["-d", name, "/etc/hostname"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    pub fn create_printer(_: &str) -> bool {
        false
    }
    pub fn remove_printer(_: &str) {}
    pub fn inject_state(_: &str, _: u32) -> bool {
        false
    }
    pub fn clear_state(_: &str) -> bool {
        false
    }
    pub fn get_default() -> Option<String> {
        None
    }
    pub fn set_default(_: &str) -> bool {
        false
    }
}

// ============================================================================
// RAII guards
// ============================================================================

/// RAII guard that removes the test printer when dropped. Drop runs
/// during stack unwinding too, so the host machine isn't left
/// polluted even when a test panics.
struct PrinterGuard {
    name: String,
}

impl PrinterGuard {
    fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Drop for PrinterGuard {
    fn drop(&mut self) {
        platform::remove_printer(&self.name);
    }
}

/// Restores the system default printer on drop. Tests that flip the
/// default must construct this BEFORE flipping so the prior default
/// is captured first.
#[allow(dead_code)] // Constructed in cfg-gated tests only.
struct DefaultPrinterGuard {
    prior: Option<String>,
}

impl DefaultPrinterGuard {
    #[allow(dead_code)]
    fn capture() -> Self {
        Self {
            prior: platform::get_default(),
        }
    }
}

impl Drop for DefaultPrinterGuard {
    fn drop(&mut self) {
        if let Some(prior) = &self.prior {
            let _ = platform::set_default(prior);
        }
    }
}

// ============================================================================
// Polling / monitor spawning / matcher helpers
// ============================================================================

/// Polls `check` every `POLL_TICK` until it returns `true` or the
/// deadline elapses. Returns the final outcome of `check`.
async fn poll_until<F>(deadline: Duration, mut check: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    while start.elapsed() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(POLL_TICK).await;
    }
    check()
}

type CapturedChanges = Arc<Mutex<Vec<PrinterChanges>>>;
type MonitorHandle = JoinHandle<Result<()>>;

/// Spawns a `monitor_printer_changes` task that pushes every observed
/// [`PrinterChanges`] into a shared vec, returning the vec, the
/// cancellation token, and the join handle.
fn spawn_monitor(
    monitor: PrinterMonitor,
    printer: &str,
) -> (CapturedChanges, CancellationToken, MonitorHandle) {
    spawn_monitor_with_interval(monitor, printer, MONITOR_INTERVAL_MS)
}

fn spawn_monitor_with_interval(
    monitor: PrinterMonitor,
    printer: &str,
    interval_ms: u64,
) -> (CapturedChanges, CancellationToken, MonitorHandle) {
    let captured: CapturedChanges = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let printer_owned = printer.to_string();

    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer_changes(
                &printer_owned,
                interval_ms,
                Some(cancel_clone),
                move |changes| {
                    captured_clone.lock().unwrap().push(changes.clone());
                },
            )
            .await
    });

    (captured, cancel, handle)
}

async fn shut_down(cancel: CancellationToken, handle: MonitorHandle) {
    cancel.cancel();
    let _ = timeout(SHUTDOWN_TIMEOUT, handle).await;
}

fn saw_change<F>(captured: &CapturedChanges, mut matcher: F) -> bool
where
    F: FnMut(&PropertyChange) -> bool,
{
    captured
        .lock()
        .unwrap()
        .iter()
        .any(|changes| changes.changes.iter().any(&mut matcher))
}

/// Returns `true` if the captured set contains a change matching the
/// requested IsOffline transition.
fn saw_is_offline_transition(captured: &CapturedChanges, old: bool, new: bool) -> bool {
    saw_change(
        captured,
        |c| matches!(c, PropertyChange::IsOffline { old: o, new: n } if *o == old && *n == new),
    )
}

#[cfg(windows)]
fn saw_state_change_to(captured: &CapturedChanges, expected: &PrinterState) -> bool {
    saw_change(
        captured,
        |c| matches!(c, PropertyChange::State { new: Some(s), .. } if s == expected),
    )
}

#[cfg(any(windows, unix))]
fn saw_status_change_either_direction(
    captured: &CapturedChanges,
    expected: &PrinterStatus,
) -> bool {
    saw_change(
        captured,
        |c| matches!(c, PropertyChange::Status { old, new } if old == expected || new == expected),
    )
}

fn saw_error_state_change_to(captured: &CapturedChanges, expected: &ErrorState) -> bool {
    saw_change(
        captured,
        |c| matches!(c, PropertyChange::ErrorState { new, .. } if new == expected),
    )
}

fn saw_is_default_transition(captured: &CapturedChanges, old: bool, new: bool) -> bool {
    saw_change(
        captured,
        |c| matches!(c, PropertyChange::IsDefault { old: o, new: n } if *o == old && *n == new),
    )
}

fn saw_name_change(captured: &CapturedChanges, old: &str, new: &str) -> bool {
    saw_change(
        captured,
        |c| matches!(c, PropertyChange::Name { old: o, new: n } if o == old && n == new),
    )
}

fn saw_property_named(captured: &CapturedChanges, property_name: &str) -> bool {
    captured
        .lock()
        .unwrap()
        .iter()
        .any(|changes| changes.has_property_change(property_name))
}

fn skip(reason: &str) {
    eprintln!(
        "[SKIP] {}: {} (run as administrator/root to exercise the full suite)",
        module_path!(),
        reason
    );
}

// ============================================================================
// Scripted backend
// ============================================================================

/// Categorical error kind used by `ScriptStep::Error`. Modeled as a
/// plain enum so `ScriptStep` stays `Clone` - `PrinterError` itself
/// holds a `std::io::Error` and isn't `Clone`. Each kind constructs a
/// fresh `PrinterError` each time it's consumed.
#[derive(Clone, Copy, Debug)]
enum ScriptErrorKind {
    Wmi,
    Cups,
    Io,
    Other,
}

impl ScriptErrorKind {
    fn build(self) -> PrinterError {
        match self {
            ScriptErrorKind::Wmi => PrinterError::WmiError("scripted-wmi".to_string()),
            ScriptErrorKind::Cups => PrinterError::CupsError("scripted-cups".to_string()),
            ScriptErrorKind::Io => PrinterError::IoError(std::io::Error::other("scripted-io")),
            ScriptErrorKind::Other => PrinterError::Other("scripted-other".to_string()),
        }
    }
}

/// One scripted backend response. The script is consumed front-to-back
/// by successive `find_printer` calls; once only the final step
/// remains it is replayed indefinitely (saturating). This lets a test
/// write `[Found, Missing]` and trust the monitor to settle on the
/// reported `Missing` state.
#[derive(Clone)]
enum ScriptStep {
    Found(Printer),
    Missing,
    Error(ScriptErrorKind),
}

struct ScriptState {
    steps: VecDeque<ScriptStep>,
    /// Last printer surfaced from a `Found` step. `list_printers`
    /// returns at most this printer so tests that call it after a few
    /// polls see something sensible.
    last_printer: Option<Printer>,
}

struct ScriptedBackend {
    state: Arc<Mutex<ScriptState>>,
}

impl ScriptedBackend {
    fn from_steps(steps: Vec<ScriptStep>) -> Self {
        let last_printer = steps.iter().rev().find_map(|step| match step {
            ScriptStep::Found(p) => Some(p.clone()),
            _ => None,
        });
        Self {
            state: Arc::new(Mutex::new(ScriptState {
                steps: steps.into(),
                last_printer,
            })),
        }
    }
}

#[async_trait]
impl PrinterBackend for ScriptedBackend {
    async fn new() -> Result<Self> {
        Ok(Self::from_steps(Vec::new()))
    }

    async fn list_printers(&self) -> Result<Vec<Printer>> {
        let state = self.state.lock().unwrap();
        Ok(state.last_printer.clone().into_iter().collect())
    }

    async fn find_printer(&self, _name: &str) -> Result<Option<Printer>> {
        let mut state = self.state.lock().unwrap();
        // Saturating consume: once we're down to the last step it
        // stays in place and is replayed on every subsequent call.
        let step = if state.steps.len() > 1 {
            state.steps.pop_front()
        } else {
            state.steps.front().cloned()
        };
        match step {
            Some(ScriptStep::Found(p)) => {
                state.last_printer = Some(p.clone());
                Ok(Some(p))
            }
            Some(ScriptStep::Missing) | None => Ok(None),
            Some(ScriptStep::Error(kind)) => Err(kind.build()),
        }
    }
}

fn make_printer(
    name: &str,
    status: PrinterStatus,
    state: Option<PrinterState>,
    error: ErrorState,
    is_offline: bool,
    is_default: bool,
) -> Printer {
    Printer::new_with_state(
        name.to_string(),
        status,
        state,
        error,
        is_offline,
        is_default,
    )
}

fn scripted_monitor(steps: Vec<ScriptStep>) -> PrinterMonitor {
    PrinterMonitor::from_backend(Arc::new(ScriptedBackend::from_steps(steps)))
}

// ============================================================================
// Existing tests (kept as-is for back-compat)
// ============================================================================

#[tokio::test]
async fn detects_printer_disappearance() {
    if !platform::create_printer(TEST_PRINTER) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER);

    let monitor = PrinterMonitor::new()
        .await
        .expect("PrinterMonitor::new must succeed on a supported platform");

    let found = monitor
        .find_printer(TEST_PRINTER)
        .await
        .expect("find_printer must not error");
    assert!(
        found.is_some(),
        "test printer should be visible immediately after creation"
    );

    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;
    platform::remove_printer(TEST_PRINTER);

    let saw_offline = poll_until(EVENT_DEADLINE, || {
        saw_is_offline_transition(&captured, false, true)
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        saw_offline,
        "monitor_printer_changes should report IsOffline:false->true when the printer is removed; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
async fn detects_printer_reappearance() {
    if !platform::create_printer(TEST_PRINTER) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER);

    let monitor = PrinterMonitor::new()
        .await
        .expect("PrinterMonitor::new must succeed on a supported platform");

    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    platform::remove_printer(TEST_PRINTER);
    let saw_offline = poll_until(EVENT_DEADLINE, || {
        saw_is_offline_transition(&captured, false, true)
    })
    .await;
    if !saw_offline {
        shut_down(cancel, handle).await;
        panic!(
            "monitor failed to report the disappearance that precedes the reappearance test; captured: {:?}",
            captured.lock().unwrap()
        );
    }

    if !platform::create_printer(TEST_PRINTER) {
        shut_down(cancel, handle).await;
        panic!("could not recreate test printer for reappearance check");
    }

    let saw_online = poll_until(EVENT_DEADLINE, || {
        saw_is_offline_transition(&captured, true, false)
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        saw_online,
        "monitor_printer_changes should report IsOffline:true->false when the printer reappears; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
async fn list_printers_observes_test_printer() {
    if !platform::create_printer(TEST_PRINTER) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let printers = monitor
        .list_printers()
        .await
        .expect("list_printers must not error on a supported platform");

    let names: Vec<&str> = printers.iter().map(|p| p.name()).collect();
    assert!(
        names.contains(&TEST_PRINTER),
        "list_printers should include the newly-created '{}'; saw: {:?}",
        TEST_PRINTER,
        names
    );
}

// ============================================================================
// Read-API coverage (real OS)
// ============================================================================

#[tokio::test]
async fn find_printer_case_insensitive() {
    if !platform::create_printer(TEST_PRINTER_CASEI) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_CASEI);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let upper = TEST_PRINTER_CASEI.to_uppercase();
    let found = monitor
        .find_printer(&upper)
        .await
        .expect("find_printer must not error");
    assert!(
        found.is_some(),
        "find_printer should match case-insensitively; query '{}' should resolve to '{}'",
        upper,
        TEST_PRINTER_CASEI
    );
}

#[tokio::test]
async fn find_printer_missing_returns_ok_none() {
    let monitor = match PrinterMonitor::new().await {
        Ok(m) => m,
        Err(_) => {
            skip("monitor init failed");
            return;
        }
    };
    let result = monitor
        .find_printer("DefinitelyNotARealPrinter_Test_99999_xyz")
        .await;
    assert!(
        matches!(result, Ok(None)),
        "find_printer of a non-existent name must return Ok(None), got: {:?}",
        result
    );
}

#[tokio::test]
async fn list_printers_after_remove_excludes() {
    if !platform::create_printer(TEST_PRINTER_LIST_AFTER_REMOVE) {
        skip("could not create test printer");
        return;
    }
    let guard = PrinterGuard::new(TEST_PRINTER_LIST_AFTER_REMOVE);

    let monitor = PrinterMonitor::new().await.expect("monitor init");

    let before: Vec<String> = monitor
        .list_printers()
        .await
        .expect("list_printers")
        .into_iter()
        .map(|p| p.name().to_string())
        .collect();
    assert!(
        before.iter().any(|n| n == TEST_PRINTER_LIST_AFTER_REMOVE),
        "printer should be listed after creation; saw: {:?}",
        before
    );

    drop(guard); // triggers remove

    // CUPS/WMI sometimes lag behind a removal. Poll briefly.
    // Inline loop because the check is async (list_printers().await)
    // and `poll_until` only accepts sync closures - block_on inside a
    // tokio task would panic with "cannot start a runtime from within
    // a runtime".
    let cleared = {
        let start = Instant::now();
        let mut cleared = false;
        while start.elapsed() < EVENT_DEADLINE {
            let names: Vec<String> = monitor
                .list_printers()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.name().to_string())
                .collect();
            if !names.iter().any(|n| n == TEST_PRINTER_LIST_AFTER_REMOVE) {
                cleared = true;
                break;
            }
            tokio::time::sleep(POLL_TICK).await;
        }
        cleared
    };

    assert!(
        cleared,
        "list_printers should stop reporting '{}' after Remove-Printer",
        TEST_PRINTER_LIST_AFTER_REMOVE
    );
}

#[tokio::test]
async fn printer_summary_includes_test_printer() {
    if !platform::create_printer(TEST_PRINTER_SUMMARY) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_SUMMARY);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let summary = monitor.printer_summary().await.expect("printer_summary");

    let entry = summary.get(TEST_PRINTER_SUMMARY);
    assert!(
        entry.is_some(),
        "printer_summary should include '{}'; got keys: {:?}",
        TEST_PRINTER_SUMMARY,
        summary.keys().collect::<Vec<_>>()
    );
    let entry = entry.unwrap();
    assert!(
        !entry.is_offline,
        "freshly-created test printer should not be offline; summary: {:?}",
        entry
    );
    assert!(
        !entry.has_error,
        "freshly-created test printer should not report an error; summary: {:?}",
        entry
    );
}

#[tokio::test]
async fn printer_all_accessors_do_not_panic() {
    if !platform::create_printer(TEST_PRINTER_ACCESSORS) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_ACCESSORS);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let printer = monitor
        .find_printer(TEST_PRINTER_ACCESSORS)
        .await
        .expect("find_printer")
        .expect("printer should be found");

    // Smoke-call every public accessor. The point is to flush out any
    // accessor that would panic on a real freshly-created printer.
    let _ = printer.name();
    let _ = printer.status();
    let _ = printer.state();
    let _ = printer.status_description();
    let _ = printer.error_state();
    let _ = printer.error_description();
    let _ = printer.is_offline();
    let _ = printer.is_default();
    let _ = printer.has_error();
    let _ = printer.printer_status_code();
    let _ = printer.printer_state_code();
    let _ = printer.detected_error_state_code();
    let _ = printer.extended_detected_error_state_code();
    let _ = printer.extended_printer_status_code();
    let _ = printer.wmi_status();
    let _ = printer.printer_status_description();
    let _ = printer.printer_state_description();
    let _ = printer.detected_error_state_description();
    let _ = printer.extended_detected_error_state_description();
    let _ = printer.extended_printer_status_description();

    // Platform-specific assertions: WMI exposes raw codes, CUPS does not.
    #[cfg(windows)]
    assert!(
        printer.printer_status_code().is_some(),
        "Windows backend should populate printer_status_code"
    );
    #[cfg(unix)]
    assert!(
        printer.printer_status_code().is_none(),
        "Linux backend leaves raw WMI codes as None"
    );

    assert_eq!(printer.name(), TEST_PRINTER_ACCESSORS);
}

// ============================================================================
// monitor_printer (legacy callback) - real OS
// ============================================================================

#[tokio::test]
async fn monitor_printer_fires_initial_snapshot() {
    if !platform::create_printer(TEST_PRINTER_INITIAL_SNAPSHOT) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_INITIAL_SNAPSHOT);

    let monitor = PrinterMonitor::new().await.expect("monitor init");

    let captures: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    let captures_clone = Arc::clone(&captures);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let name = TEST_PRINTER_INITIAL_SNAPSHOT.to_string();

    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer(
                &name,
                MONITOR_INTERVAL_MS,
                Some(cancel_clone),
                move |current, previous| {
                    captures_clone
                        .lock()
                        .unwrap()
                        .push((current.name().to_string(), previous.is_some()));
                },
            )
            .await
    });

    let got_initial = poll_until(EVENT_DEADLINE, || {
        captures
            .lock()
            .unwrap()
            .iter()
            .any(|(_, had_prev)| !*had_prev)
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        got_initial,
        "monitor_printer should fire with previous=None on the initial snapshot; captures: {:?}",
        captures.lock().unwrap()
    );
}

// ============================================================================
// monitor_multiple_printers - real OS + scripted
// ============================================================================

#[tokio::test]
async fn monitor_multiple_printers_fires_per_printer() {
    if !platform::create_printer(TEST_PRINTER_A) || !platform::create_printer(TEST_PRINTER_B) {
        skip("could not create both A and B test printers");
        return;
    }
    let _guard_a = PrinterGuard::new(TEST_PRINTER_A);
    let _guard_b = PrinterGuard::new(TEST_PRINTER_B);

    let monitor = PrinterMonitor::new().await.expect("monitor init");

    let captured: CapturedChanges = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let names = vec![TEST_PRINTER_A.to_string(), TEST_PRINTER_B.to_string()];

    let handle = tokio::spawn(async move {
        monitor
            .monitor_multiple_printers(
                names,
                MONITOR_INTERVAL_MS,
                Some(cancel_clone),
                move |changes| {
                    captured_clone.lock().unwrap().push(changes.clone());
                },
            )
            .await
    });

    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;
    platform::remove_printer(TEST_PRINTER_A);

    let saw_a_offline = poll_until(EVENT_DEADLINE, || {
        captured.lock().unwrap().iter().any(|changes| {
            changes.printer_name == TEST_PRINTER_A
                && changes
                    .changes
                    .iter()
                    .any(|c| matches!(c, PropertyChange::IsOffline { new: true, .. }))
        })
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        saw_a_offline,
        "A removal should fire a per-printer IsOffline change for A; captured: {:?}",
        captured.lock().unwrap()
    );
    // B should NOT have fired an IsOffline change - it stayed present.
    let saw_b_offline = captured.lock().unwrap().iter().any(|changes| {
        changes.printer_name == TEST_PRINTER_B
            && changes
                .changes
                .iter()
                .any(|c| matches!(c, PropertyChange::IsOffline { new: true, .. }))
    });
    assert!(
        !saw_b_offline,
        "B was never removed, so the multi-printer monitor should not report B as offline"
    );
}

#[tokio::test]
async fn monitor_multiple_printers_cancellation_returns_promptly() {
    let monitor = match PrinterMonitor::new().await {
        Ok(m) => m,
        Err(_) => {
            skip("monitor init failed");
            return;
        }
    };

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let names = vec![
        "NonExistentPrinter_Cancel_X".to_string(),
        "NonExistentPrinter_Cancel_Y".to_string(),
    ];

    let handle = tokio::spawn(async move {
        monitor
            .monitor_multiple_printers(names, 1000, Some(cancel_clone), |_| {})
            .await
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    cancel.cancel();

    let result = timeout(SHUTDOWN_TIMEOUT, handle).await;
    assert!(
        result.is_ok(),
        "multi-printer monitor should honour cancellation within SHUTDOWN_TIMEOUT"
    );
}

#[tokio::test]
async fn scripted_multiple_printers_aborts_siblings_on_error() {
    // One printer's script returns errors past the threshold; the other
    // would happily report Found forever. The errors should propagate
    // out of monitor_multiple_printers, with siblings aborted via
    // abort_all (no orphaned pollers).
    let bad_monitor = scripted_monitor(vec![
        ScriptStep::Error(ScriptErrorKind::Wmi);
        (MAX_CONSECUTIVE_MONITOR_ERRORS + 2) as usize
    ]);

    let names = vec!["bad".to_string(), "good".to_string()];
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // The monitor under test only sees one backend; with one bad
    // printer (returning errors regardless of name) we exercise the
    // abort-on-error path.
    let handle = tokio::spawn(async move {
        bad_monitor
            .monitor_multiple_printers(names, SCRIPTED_INTERVAL_MS, Some(cancel_clone), |_| {})
            .await
    });

    let result = timeout(SCRIPTED_EVENT_DEADLINE, handle)
        .await
        .expect("monitor_multiple_printers should resolve after consecutive errors")
        .expect("task should not panic");

    cancel.cancel(); // safety net for any lingering task

    assert!(
        matches!(result, Err(PrinterError::WmiError(_))),
        "monitor_multiple_printers should surface the scripted WmiError once the threshold is exceeded; got: {:?}",
        result
    );
}

// ============================================================================
// Cancellation - integration-level
// ============================================================================

#[tokio::test]
async fn cancellation_during_sleep_returns_promptly() {
    if !platform::create_printer(TEST_PRINTER_CANCEL) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_CANCEL);

    let monitor = PrinterMonitor::new().await.expect("monitor init");

    // Long interval so the cancel-during-sleep path is the one we hit.
    let (_captured, cancel, handle) =
        spawn_monitor_with_interval(monitor, TEST_PRINTER_CANCEL, 30_000);

    tokio::time::sleep(Duration::from_millis(200)).await;
    let start = Instant::now();
    cancel.cancel();
    let res = timeout(SHUTDOWN_TIMEOUT, handle).await;
    let elapsed = start.elapsed();

    assert!(
        res.is_ok(),
        "monitor task should complete within SHUTDOWN_TIMEOUT after cancel"
    );
    assert!(
        elapsed < SHUTDOWN_TIMEOUT,
        "cancellation during sleep took too long: {:?}",
        elapsed
    );
}

// ============================================================================
// Windows real-OS state injection
// ============================================================================

#[cfg(windows)]
async fn inject_state_to_state_test(name: &str, flags: u32, expected: PrinterState) {
    if !platform::create_printer(name) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(name);
    let _ = platform::clear_state(name);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, name);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::inject_state(name, flags) {
        let _ = platform::clear_state(name);
        shut_down(cancel, handle).await;
        skip(&format!(
            "could not inject status flags 0x{:x} on '{}' (likely lacks Manage Printer rights)",
            flags, name
        ));
        return;
    }

    let saw = poll_until(EVENT_DEADLINE, || saw_state_change_to(&captured, &expected)).await;

    let _ = platform::clear_state(name);
    shut_down(cancel, handle).await;

    assert!(
        saw,
        "expected PrinterState::{:?} after injecting flags 0x{:x}; captured: {:?}",
        expected,
        flags,
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_paper_out() {
    inject_state_to_state_test(
        TEST_PRINTER_PAPER_OUT,
        PRINTER_STATUS_PAPER_OUT,
        PrinterState::PaperOut,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_door_open() {
    inject_state_to_state_test(
        TEST_PRINTER_DOOR_OPEN,
        PRINTER_STATUS_DOOR_OPEN,
        PrinterState::DoorOpen,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_paper_jam() {
    inject_state_to_state_test(
        TEST_PRINTER_PAPER_JAM,
        PRINTER_STATUS_PAPER_JAM,
        PrinterState::PaperJam,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_toner_low() {
    inject_state_to_state_test(
        TEST_PRINTER_TONER_LOW,
        PRINTER_STATUS_TONER_LOW,
        PrinterState::TonerLow,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_no_toner() {
    inject_state_to_state_test(
        TEST_PRINTER_NO_TONER,
        PRINTER_STATUS_NO_TONER,
        PrinterState::NoToner,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_output_bin_full() {
    inject_state_to_state_test(
        TEST_PRINTER_OUTPUT_BIN,
        PRINTER_STATUS_OUTPUT_BIN_FULL,
        PrinterState::OutputBinFull,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_out_of_memory() {
    inject_state_to_state_test(
        TEST_PRINTER_OOM,
        PRINTER_STATUS_OUT_OF_MEMORY,
        PrinterState::OutOfMemory,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_user_intervention() {
    inject_state_to_state_test(
        TEST_PRINTER_USER_INT,
        PRINTER_STATUS_USER_INTERVENTION,
        PrinterState::UserInterventionRequired,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_error() {
    inject_state_to_state_test(
        TEST_PRINTER_ERROR,
        PRINTER_STATUS_ERROR,
        PrinterState::Error,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_inject_combined_flags_priority_wins() {
    // DoorOpen|PaperOut: DoorOpen has higher priority in
    // PrinterState::from_u32, so the surfaced enum should be DoorOpen,
    // not PaperOut.
    inject_state_to_state_test(
        TEST_PRINTER_COMBINED,
        PRINTER_STATUS_DOOR_OPEN | PRINTER_STATUS_PAPER_OUT,
        PrinterState::DoorOpen,
    )
    .await;
}

#[tokio::test]
#[cfg(windows)]
async fn windows_pause_resume_changes_state() {
    if !platform::create_printer(TEST_PRINTER_PAUSE_RESUME) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_PAUSE_RESUME);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_PAUSE_RESUME);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::pause_printer(TEST_PRINTER_PAUSE_RESUME) {
        let _ = platform::resume_printer(TEST_PRINTER_PAUSE_RESUME);
        shut_down(cancel, handle).await;
        skip("Suspend-Printer not available");
        return;
    }

    let saw_paused = poll_until(EVENT_DEADLINE, || {
        saw_state_change_to(&captured, &PrinterState::Paused)
    })
    .await;

    let _ = platform::resume_printer(TEST_PRINTER_PAUSE_RESUME);
    shut_down(cancel, handle).await;

    assert!(
        saw_paused,
        "Suspend-Printer should surface as PrinterState::Paused; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(windows)]
async fn windows_set_default_toggles_is_default() {
    if !platform::create_printer(TEST_PRINTER_SET_DEFAULT) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_SET_DEFAULT);
    let _restore = DefaultPrinterGuard::capture();

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_SET_DEFAULT);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::set_default(TEST_PRINTER_SET_DEFAULT) {
        shut_down(cancel, handle).await;
        skip("SetDefaultPrinter invocation failed");
        return;
    }

    let saw = poll_until(EVENT_DEADLINE, || {
        saw_is_default_transition(&captured, false, true)
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        saw,
        "set-default should surface as IsDefault:false->true; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(windows)]
async fn windows_rename_visible_via_list_printers() {
    if !platform::create_printer(TEST_PRINTER_RENAME_SRC) {
        skip("could not create test printer");
        return;
    }
    // Guard both names: source disappears after rename; dest needs cleanup too.
    let _guard_src = PrinterGuard::new(TEST_PRINTER_RENAME_SRC);
    let _guard_dst = PrinterGuard::new(TEST_PRINTER_RENAME_DST);

    let monitor = PrinterMonitor::new().await.expect("monitor init");

    let before: Vec<String> = monitor
        .list_printers()
        .await
        .expect("list_printers")
        .into_iter()
        .map(|p| p.name().to_string())
        .collect();
    assert!(
        before.iter().any(|n| n == TEST_PRINTER_RENAME_SRC),
        "source printer should be listed before rename"
    );

    if !platform::rename_printer(TEST_PRINTER_RENAME_SRC, TEST_PRINTER_RENAME_DST) {
        skip("Rename-Printer not available");
        return;
    }

    // Inline async poll: see list_printers_after_remove_excludes for why
    // we can't call list_printers from inside a sync `poll_until`.
    let saw_dst = {
        let start = Instant::now();
        let mut saw = false;
        while start.elapsed() < EVENT_DEADLINE {
            let names: Vec<String> = monitor
                .list_printers()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.name().to_string())
                .collect();
            if names.iter().any(|n| n == TEST_PRINTER_RENAME_DST)
                && !names.iter().any(|n| n == TEST_PRINTER_RENAME_SRC)
            {
                saw = true;
                break;
            }
            tokio::time::sleep(POLL_TICK).await;
        }
        saw
    };

    assert!(
        saw_dst,
        "list_printers should reflect the renamed printer; expected '{}' present and '{}' gone",
        TEST_PRINTER_RENAME_DST, TEST_PRINTER_RENAME_SRC
    );
}

#[tokio::test]
#[cfg(windows)]
async fn windows_submit_job_best_effort_printing() {
    if !platform::create_printer(TEST_PRINTER_SUBMIT_JOB) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_SUBMIT_JOB);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_SUBMIT_JOB);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::submit_job(TEST_PRINTER_SUBMIT_JOB) {
        shut_down(cancel, handle).await;
        skip("Out-Printer unavailable or job submission failed");
        return;
    }

    // Best-effort: the job goes to nul: so it might complete inside one
    // poll interval. We just check we saw _some_ Status transition that
    // mentioned Printing, but don't fail if not.
    let _ = poll_until(EVENT_DEADLINE, || {
        saw_status_change_either_direction(&captured, &PrinterStatus::Printing)
    })
    .await;

    shut_down(cancel, handle).await;
    // Intentionally no hard assert - documented as best-effort.
}

// ============================================================================
// Linux real-OS
// ============================================================================

#[tokio::test]
#[cfg(unix)]
async fn linux_cups_disable_marks_offline() {
    if !platform::create_printer(TEST_PRINTER_CUPS_DISABLE) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_CUPS_DISABLE);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_CUPS_DISABLE);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::cups_disable(TEST_PRINTER_CUPS_DISABLE) {
        let _ = platform::cups_enable(TEST_PRINTER_CUPS_DISABLE);
        shut_down(cancel, handle).await;
        skip("cupsdisable unavailable");
        return;
    }

    let saw = poll_until(EVENT_DEADLINE, || {
        saw_is_offline_transition(&captured, false, true)
    })
    .await;

    let _ = platform::cups_enable(TEST_PRINTER_CUPS_DISABLE);
    shut_down(cancel, handle).await;

    assert!(
        saw,
        "cupsdisable should surface IsOffline:false->true; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(unix)]
async fn linux_cups_enable_restores_online() {
    if !platform::create_printer(TEST_PRINTER_CUPS_ENABLE) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_CUPS_ENABLE);

    let monitor = PrinterMonitor::new().await.expect("monitor init");

    if !platform::cups_disable(TEST_PRINTER_CUPS_ENABLE) {
        skip("cupsdisable unavailable");
        return;
    }

    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_CUPS_ENABLE);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::cups_enable(TEST_PRINTER_CUPS_ENABLE) {
        shut_down(cancel, handle).await;
        skip("cupsenable unavailable");
        return;
    }

    let saw = poll_until(EVENT_DEADLINE, || {
        saw_is_offline_transition(&captured, true, false)
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        saw,
        "cupsenable should surface IsOffline:true->false; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(unix)]
async fn linux_set_default_toggles_is_default() {
    if !platform::create_printer(TEST_PRINTER_LINUX_DEFAULT) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_LINUX_DEFAULT);
    let _restore = DefaultPrinterGuard::capture();

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_LINUX_DEFAULT);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::set_default(TEST_PRINTER_LINUX_DEFAULT) {
        shut_down(cancel, handle).await;
        skip("lpadmin -d failed");
        return;
    }

    let saw = poll_until(EVENT_DEADLINE, || {
        saw_is_default_transition(&captured, false, true)
    })
    .await;

    shut_down(cancel, handle).await;

    assert!(
        saw,
        "lpadmin -d should surface IsDefault:false->true; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(unix)]
async fn linux_submit_job_best_effort_printing() {
    if !platform::create_printer(TEST_PRINTER_LINUX_SUBMIT) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard::new(TEST_PRINTER_LINUX_SUBMIT);

    let monitor = PrinterMonitor::new().await.expect("monitor init");
    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER_LINUX_SUBMIT);
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    if !platform::submit_job(TEST_PRINTER_LINUX_SUBMIT) {
        shut_down(cancel, handle).await;
        skip("lp submission failed");
        return;
    }

    // Best-effort - the file:/// destination drains the job instantly.
    let _ = poll_until(EVENT_DEADLINE, || {
        saw_status_change_either_direction(&captured, &PrinterStatus::Printing)
    })
    .await;

    shut_down(cancel, handle).await;
}

// ============================================================================
// Scripted-backend tests
// ============================================================================

async fn spawn_scripted_monitor(
    monitor: PrinterMonitor,
    printer: &str,
) -> (CapturedChanges, CancellationToken, MonitorHandle) {
    spawn_monitor_with_interval(monitor, printer, SCRIPTED_INTERVAL_MS)
}

async fn shut_down_scripted(cancel: CancellationToken, handle: MonitorHandle) -> Result<()> {
    cancel.cancel();
    timeout(SHUTDOWN_TIMEOUT, handle)
        .await
        .expect("scripted monitor should honour cancel within SHUTDOWN_TIMEOUT")
        .expect("scripted monitor task should not panic")
}

#[tokio::test]
async fn scripted_initial_state_is_silent() {
    let printer = make_printer(
        "scripted",
        PrinterStatus::Idle,
        Some(PrinterState::None),
        ErrorState::NoError,
        false,
        false,
    );
    let monitor = scripted_monitor(vec![ScriptStep::Found(printer)]);

    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "scripted").await;
    tokio::time::sleep(SCRIPTED_INITIAL_CAPTURE_DELAY).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _ = shut_down_scripted(cancel, handle).await;
    let captured = captured.lock().unwrap();
    assert!(
        captured.is_empty(),
        "initial state must be silent; saw: {:?}",
        captured
    );
}

#[tokio::test]
async fn scripted_status_transitions() {
    // Idle -> Printing
    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Printing,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
    ]);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;

    let saw = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        saw_change(&captured, |c| {
            matches!(
                c,
                PropertyChange::Status {
                    old: PrinterStatus::Idle,
                    new: PrinterStatus::Printing
                }
            )
        })
    })
    .await;
    let _ = shut_down_scripted(cancel, handle).await;
    assert!(
        saw,
        "scripted Idle->Printing transition should fire as PropertyChange::Status; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
async fn scripted_error_state_transitions() {
    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::Jammed,
            false,
            false,
        )),
    ]);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;

    let saw = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        saw_error_state_change_to(&captured, &ErrorState::Jammed)
    })
    .await;
    let _ = shut_down_scripted(cancel, handle).await;
    assert!(saw, "scripted ErrorState change to Jammed should fire");
}

#[tokio::test]
async fn scripted_name_change() {
    // A monitor that watches by name would never see Name change (the
    // baseline is fixed), but compare_with itself does surface the
    // field. We exercise that via the scripted backend by returning
    // two different names in a row.
    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
        ScriptStep::Found(make_printer(
            "p-renamed",
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
    ]);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;
    let saw = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        saw_name_change(&captured, "p", "p-renamed")
    })
    .await;
    let _ = shut_down_scripted(cancel, handle).await;
    assert!(saw, "scripted name change should fire PropertyChange::Name");
}

#[tokio::test]
async fn scripted_disappear_reappear_baseline_invariant() {
    // [Found, Missing, Missing, Found(different)] must produce:
    // - exactly one IsOffline:false->true (gated by was_present_last_poll)
    // - on reappearance, IsOffline:true->false PLUS the property delta
    //   that accumulated during the gap (here: ErrorState change).
    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
        ScriptStep::Missing,
        ScriptStep::Missing,
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::Jammed,
            false,
            false,
        )),
    ]);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;

    let saw_reappear = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        saw_is_offline_transition(&captured, true, false)
    })
    .await;

    let _ = shut_down_scripted(cancel, handle).await;
    let captured_vec = captured.lock().unwrap().clone();

    assert!(
        saw_reappear,
        "reappearance must surface IsOffline:true->false; captured: {:?}",
        captured_vec
    );

    let offline_true_events: usize = captured_vec
        .iter()
        .flat_map(|c| c.changes.iter())
        .filter(|c| matches!(c, PropertyChange::IsOffline { new: true, .. }))
        .count();
    assert_eq!(
        offline_true_events, 1,
        "fresh-disappearance should fire exactly once per gap; saw {} events; captured: {:?}",
        offline_true_events, captured_vec
    );

    let saw_error_delta = captured_vec.iter().any(|c| {
        c.changes.iter().any(|change| {
            matches!(
                change,
                PropertyChange::ErrorState {
                    new: ErrorState::Jammed,
                    ..
                }
            )
        })
    });
    assert!(
        saw_error_delta,
        "reappearance must surface property deltas accumulated during the gap (B4); captured: {:?}",
        captured_vec
    );
}

#[tokio::test]
async fn scripted_transient_errors_recover_below_threshold() {
    let below = (MAX_CONSECUTIVE_MONITOR_ERRORS - 1) as usize;
    let mut steps: Vec<ScriptStep> = (0..below)
        .map(|_| ScriptStep::Error(ScriptErrorKind::Wmi))
        .collect();
    steps.push(ScriptStep::Found(make_printer(
        "p",
        PrinterStatus::Idle,
        None,
        ErrorState::NoError,
        false,
        false,
    )));

    let monitor = scripted_monitor(steps);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;

    // Wait long enough to consume the errors + the successful poll +
    // a couple extra polls confirming silence.
    tokio::time::sleep(Duration::from_millis(
        SCRIPTED_INTERVAL_MS * (below as u64 + 4),
    ))
    .await;

    let result = shut_down_scripted(cancel, handle).await;
    assert!(
        result.is_ok(),
        "monitor must survive {} consecutive errors (below threshold of {}); got: {:?}",
        below,
        MAX_CONSECUTIVE_MONITOR_ERRORS,
        result
    );
    assert!(
        captured.lock().unwrap().is_empty(),
        "no callback should fire when only the initial state was eventually captured; saw: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
async fn scripted_fatal_after_max_consecutive_errors() {
    let monitor = scripted_monitor(vec![
        ScriptStep::Error(ScriptErrorKind::Wmi);
        (MAX_CONSECUTIVE_MONITOR_ERRORS + 2) as usize
    ]);

    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer_changes("any", SCRIPTED_INTERVAL_MS, None, |_| {})
            .await
    });

    let result = timeout(SCRIPTED_EVENT_DEADLINE, handle)
        .await
        .expect("monitor must return after exceeding error threshold")
        .expect("task should not panic");

    assert!(
        matches!(result, Err(PrinterError::WmiError(_))),
        "monitor must surface the underlying error after {} consecutive failures; got: {:?}",
        MAX_CONSECUTIVE_MONITOR_ERRORS,
        result
    );
}

#[tokio::test]
async fn scripted_intermittent_errors_reset_counter() {
    // 4 errors -> Found (resets counter) -> 5 errors -> fatal.
    let pre = (MAX_CONSECUTIVE_MONITOR_ERRORS - 1) as usize;
    let post = MAX_CONSECUTIVE_MONITOR_ERRORS as usize;
    let mut steps: Vec<ScriptStep> = (0..pre)
        .map(|_| ScriptStep::Error(ScriptErrorKind::Wmi))
        .collect();
    steps.push(ScriptStep::Found(make_printer(
        "p",
        PrinterStatus::Idle,
        None,
        ErrorState::NoError,
        false,
        false,
    )));
    steps.extend((0..post).map(|_| ScriptStep::Error(ScriptErrorKind::Wmi)));

    let monitor = scripted_monitor(steps);
    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer_changes("any", SCRIPTED_INTERVAL_MS, None, |_| {})
            .await
    });

    let result = timeout(SCRIPTED_EVENT_DEADLINE, handle)
        .await
        .expect("monitor must finish after the second error burst")
        .expect("task should not panic");

    assert!(
        matches!(result, Err(PrinterError::WmiError(_))),
        "monitor must reset its error counter on success and then fail again after a fresh streak of {} errors; got: {:?}",
        MAX_CONSECUTIVE_MONITOR_ERRORS,
        result
    );
}

#[tokio::test]
async fn scripted_monitor_property_filter() {
    // Script flips both Status and IsOffline at once; the filter for
    // Status must only see Status changes, never IsOffline.
    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
        )),
        ScriptStep::Found(make_printer(
            "p",
            PrinterStatus::Printing,
            None,
            ErrorState::NoError,
            true,
            false,
        )),
    ]);

    let captured: Arc<Mutex<Vec<PropertyChange>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    let handle = tokio::spawn(async move {
        monitor
            .monitor_property(
                "p",
                MonitorableProperty::Status,
                SCRIPTED_INTERVAL_MS,
                Some(cancel_clone),
                move |change| {
                    captured_clone.lock().unwrap().push(change.clone());
                },
            )
            .await
    });

    let saw_status = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        captured
            .lock()
            .unwrap()
            .iter()
            .any(|c| matches!(c, PropertyChange::Status { .. }))
    })
    .await;

    cancel.cancel();
    let _ = timeout(SHUTDOWN_TIMEOUT, handle).await;

    assert!(saw_status, "filter should surface the Status change");
    let none_off = captured
        .lock()
        .unwrap()
        .iter()
        .all(|c| !matches!(c, PropertyChange::IsOffline { .. }));
    assert!(
        none_off,
        "Status-only filter must drop IsOffline changes; saw: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(windows)]
async fn scripted_raw_code_changes() {
    // Round-trip two Win32Printer-shaped Printer instances through the
    // public WMI constructor so we exercise the raw-code paths.
    use printer_event_handler::printer::WmiStatusCodes;

    let make = |status: u32, error: u32| -> Printer {
        Printer::new_with_wmi(
            "p".to_string(),
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
            WmiStatusCodes {
                printer_status_code: Some(status),
                printer_state_code: Some(0),
                detected_error_state_code: Some(error),
                extended_detected_error_state_code: Some(error),
                extended_printer_status_code: Some(status),
                wmi_status: Some("OK".to_string()),
            },
        )
    };

    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make(3, 2)), // Idle, NoError
        ScriptStep::Found(make(4, 8)), // Printing, Jammed
    ]);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;

    let saw = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        saw_property_named(&captured, "PrinterStatusCode")
            && saw_property_named(&captured, "DetectedErrorStateCode")
            && saw_property_named(&captured, "ExtendedPrinterStatusCode")
            && saw_property_named(&captured, "ExtendedDetectedErrorStateCode")
    })
    .await;
    let _ = shut_down_scripted(cancel, handle).await;

    assert!(
        saw,
        "all four raw-code PropertyChange variants must fire when the WMI codes change; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
#[cfg(windows)]
async fn scripted_wmi_status_string_changes() {
    use printer_event_handler::printer::WmiStatusCodes;

    let make = |wmi_status: &str| -> Printer {
        Printer::new_with_wmi(
            "p".to_string(),
            PrinterStatus::Idle,
            None,
            ErrorState::NoError,
            false,
            false,
            WmiStatusCodes {
                printer_status_code: Some(3),
                printer_state_code: Some(0),
                detected_error_state_code: Some(2),
                extended_detected_error_state_code: Some(2),
                extended_printer_status_code: Some(3),
                wmi_status: Some(wmi_status.to_string()),
            },
        )
    };

    let monitor = scripted_monitor(vec![
        ScriptStep::Found(make("OK")),
        ScriptStep::Found(make("Degraded")),
    ]);
    let (captured, cancel, handle) = spawn_scripted_monitor(monitor, "p").await;

    let saw = poll_until(SCRIPTED_EVENT_DEADLINE, || {
        saw_property_named(&captured, "WmiStatus")
    })
    .await;
    let _ = shut_down_scripted(cancel, handle).await;

    assert!(
        saw,
        "WmiStatus PropertyChange must fire on a status string change; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
async fn scripted_error_kinds_surface_each_variant() {
    for kind in [
        ScriptErrorKind::Wmi,
        ScriptErrorKind::Cups,
        ScriptErrorKind::Io,
        ScriptErrorKind::Other,
    ] {
        let monitor = scripted_monitor(vec![
            ScriptStep::Error(kind);
            (MAX_CONSECUTIVE_MONITOR_ERRORS + 1) as usize
        ]);
        let handle = tokio::spawn(async move {
            monitor
                .monitor_printer_changes("any", SCRIPTED_INTERVAL_MS, None, |_| {})
                .await
        });
        let result = timeout(SCRIPTED_EVENT_DEADLINE, handle)
            .await
            .unwrap_or_else(|_| panic!("monitor should finish for kind {:?}", kind))
            .expect("task should not panic");

        let matched = matches!(
            (kind, &result),
            (ScriptErrorKind::Wmi, Err(PrinterError::WmiError(_)))
                | (ScriptErrorKind::Cups, Err(PrinterError::CupsError(_)))
                | (ScriptErrorKind::Io, Err(PrinterError::IoError(_)))
                | (ScriptErrorKind::Other, Err(PrinterError::Other(_)))
        );
        assert!(
            matched,
            "{:?} should propagate the matching PrinterError variant; got: {:?}",
            kind, result
        );
    }
}

// ============================================================================
// PrinterError surface
// ============================================================================

#[test]
fn printer_error_display_strings_cover_all_variants() {
    assert_eq!(
        PrinterError::WmiError("x".to_string()).to_string(),
        "WMI error: x"
    );
    assert_eq!(
        PrinterError::CupsError("x".to_string()).to_string(),
        "CUPS error: x"
    );
    assert_eq!(
        PrinterError::PrinterNotFound("x".to_string()).to_string(),
        "Printer 'x' not found"
    );
    assert_eq!(
        PrinterError::PlatformNotSupported.to_string(),
        "This platform is not supported"
    );
    assert!(
        PrinterError::IoError(std::io::Error::other("x"))
            .to_string()
            .contains("I/O error")
    );
    assert_eq!(PrinterError::Other("x".to_string()).to_string(), "x");
}
