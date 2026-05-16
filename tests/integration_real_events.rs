//! End-to-end integration tests that exercise real OS-level printer
//! observation.
//!
//! These tests create a real printer on the host system, run
//! [`PrinterMonitor::monitor_printer_changes`] against it, then trigger
//! real disappearance and reappearance events and assert that the
//! library's callback fires with the expected
//! [`PropertyChange::IsOffline`] transitions.
//!
//! ## Requirements
//!
//! - **Windows**: Administrator privileges. The tests invoke
//!   `Add-Printer` / `Remove-Printer` via PowerShell. The `nul:` port
//!   and the `Generic / Text Only` driver must be available (both
//!   ship with Windows).
//! - **Linux**: Root or `lpadmin` group membership. The tests invoke
//!   `lpadmin` to register a CUPS printer pointing at
//!   `file:///dev/null`.
//!
//! When printer creation fails (typically because of missing
//! privileges) each test prints a skip message to stderr and returns
//! successfully rather than failing - that way `cargo test --
//! --ignored` does not turn into a privilege check.
//!
//! ## Running
//!
//! ```text
//! cargo test --test integration_real_events -- --ignored
//! ```
//!
//! These tests are `#[ignore]` by default so the regular `cargo test`
//! run is unaffected.

use printer_event_handler::{
    CancellationToken, PrinterChanges, PrinterMonitor, PropertyChange,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Test printer name. Must not collide with any real printer on the
/// host system - the suffix is deliberately unusual.
const TEST_PRINTER: &str = "RustPrinterEventHandlerTest_42";
/// Poll interval the library uses while watching the test printer.
/// Tight enough that disappearance/reappearance events surface within
/// a few seconds.
const MONITOR_INTERVAL_MS: u64 = 500;
/// Upper bound on how long we wait for an event to appear after we
/// trigger it. WMI/CUPS aren't instant; this is intentionally
/// generous.
const EVENT_DEADLINE: Duration = Duration::from_secs(15);
/// How long to sleep after spawning the monitor before triggering the
/// first event - lets the monitor record the baseline (which is
/// silent by design).
const INITIAL_CAPTURE_DELAY: Duration = Duration::from_millis(1_500);
/// Max wait for the monitor task to honour cancellation before we
/// give up on a clean shutdown.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// Cadence for `poll_until` - small enough to not blunt EVENT_DEADLINE.
const POLL_TICK: Duration = Duration::from_millis(100);

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

    /// Returns `true` if the printer was created (or already exists).
    pub fn create_printer(name: &str) -> bool {
        // The `nul:` port might already be present from a prior run; we
        // try-add silently so a duplicate doesn't poison the run.
        powershell("Add-PrinterPort -Name 'nul:' -ErrorAction SilentlyContinue");
        powershell(&format!(
            "Add-Printer -Name '{}' -DriverName 'Generic / Text Only' -PortName 'nul:'",
            name
        ))
    }

    pub fn remove_printer(name: &str) {
        let _ = powershell(&format!(
            "Remove-Printer -Name '{}' -ErrorAction SilentlyContinue",
            name
        ));
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
}

#[cfg(not(any(windows, unix)))]
mod platform {
    pub fn create_printer(_: &str) -> bool {
        false
    }
    pub fn remove_printer(_: &str) {}
}

/// RAII guard that removes the test printer when dropped. Drop runs
/// during stack unwinding too, so the host machine isn't left
/// polluted even when a test panics.
struct PrinterGuard {
    name: &'static str,
}

impl Drop for PrinterGuard {
    fn drop(&mut self) {
        platform::remove_printer(self.name);
    }
}

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
type MonitorHandle = JoinHandle<printer_event_handler::Result<()>>;

/// Spawns a `monitor_printer_changes` task that pushes every observed
/// [`PrinterChanges`] into a shared vec, returning the vec, the
/// cancellation token, and the join handle.
fn spawn_monitor(
    monitor: PrinterMonitor,
    printer: &'static str,
) -> (CapturedChanges, CancellationToken, MonitorHandle) {
    let captured: Arc<Mutex<Vec<PrinterChanges>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer_changes(
                printer,
                MONITOR_INTERVAL_MS,
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

/// Returns `true` if the captured set contains a change matching the
/// requested IsOffline transition.
fn saw_is_offline_transition(captured: &CapturedChanges, old: bool, new: bool) -> bool {
    captured.lock().unwrap().iter().any(|changes| {
        changes.changes.iter().any(|change| {
            matches!(
                change,
                PropertyChange::IsOffline { old: o, new: n } if *o == old && *n == new
            )
        })
    })
}

fn skip(reason: &str) {
    eprintln!(
        "[SKIP] {}: {} (run as administrator/root to exercise the full suite)",
        module_path!(),
        reason
    );
}

#[tokio::test]
// #[ignore]
async fn detects_printer_disappearance() {
    if !platform::create_printer(TEST_PRINTER) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard { name: TEST_PRINTER };

    let monitor = PrinterMonitor::new()
        .await
        .expect("PrinterMonitor::new must succeed on a supported platform");

    // Sanity: the freshly-created printer is observable through the
    // library's read APIs before we start the monitor.
    let found = monitor
        .find_printer(TEST_PRINTER)
        .await
        .expect("find_printer must not error");
    assert!(
        found.is_some(),
        "test printer should be visible immediately after creation"
    );

    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER);

    // Let the monitor record the baseline silently.
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    // Trigger the disappearance.
    platform::remove_printer(TEST_PRINTER);

    let saw_offline =
        poll_until(EVENT_DEADLINE, || saw_is_offline_transition(&captured, false, true)).await;

    shut_down(cancel, handle).await;

    assert!(
        saw_offline,
        "monitor_printer_changes should report IsOffline:false->true when the printer is removed; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
// #[ignore]
async fn detects_printer_reappearance() {
    if !platform::create_printer(TEST_PRINTER) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard { name: TEST_PRINTER };

    let monitor = PrinterMonitor::new()
        .await
        .expect("PrinterMonitor::new must succeed on a supported platform");

    let (captured, cancel, handle) = spawn_monitor(monitor, TEST_PRINTER);

    // Baseline.
    tokio::time::sleep(INITIAL_CAPTURE_DELAY).await;

    // 1. Disappearance.
    platform::remove_printer(TEST_PRINTER);
    let saw_offline =
        poll_until(EVENT_DEADLINE, || saw_is_offline_transition(&captured, false, true)).await;
    if !saw_offline {
        shut_down(cancel, handle).await;
        panic!(
            "monitor failed to report the disappearance that precedes the reappearance test; captured: {:?}",
            captured.lock().unwrap()
        );
    }

    // 2. Reappearance.
    if !platform::create_printer(TEST_PRINTER) {
        shut_down(cancel, handle).await;
        panic!("could not recreate test printer for reappearance check");
    }

    let saw_online =
        poll_until(EVENT_DEADLINE, || saw_is_offline_transition(&captured, true, false)).await;

    shut_down(cancel, handle).await;

    assert!(
        saw_online,
        "monitor_printer_changes should report IsOffline:true->false when the printer reappears; captured: {:?}",
        captured.lock().unwrap()
    );
}

#[tokio::test]
// #[ignore]
async fn list_printers_observes_test_printer() {
    if !platform::create_printer(TEST_PRINTER) {
        skip("could not create test printer");
        return;
    }
    let _guard = PrinterGuard { name: TEST_PRINTER };

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
