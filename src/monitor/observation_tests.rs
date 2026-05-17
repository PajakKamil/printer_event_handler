//! Backend-driven observation tests.
//!
//! These tests inject a scripted [`PrinterBackend`] directly into a
//! [`PrinterMonitor`] so the change-detection flow inside `monitor_*` can be
//! asserted without depending on WMI/CUPS. They cover the exact paths the
//! integration test verifies end-to-end, but with deterministic timing and
//! no admin-privilege requirement.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::backend::PrinterBackend;
use crate::{ErrorState, Printer, PrinterChanges, PrinterStatus, PropertyChange};

use super::PrinterMonitor;
use super::property::MonitorableProperty;

/// Poll interval used by the scripted-backend tests. Tight enough that
/// the tests finish quickly, loose enough that the monitor loop has
/// real work between polls.
const TEST_POLL_INTERVAL_MS: u64 = 20;
/// Upper bound for letting the scripted monitor run before cancelling.
/// Each test scripts a small number of steps; this gives the loop ample
/// time to visit each one.
const TEST_RUN_DURATION_MS: u64 = 400;
/// Max wait for the monitor task to honour cancellation.
const TEST_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Backend that returns a scripted sequence of `find_printer` responses.
/// Each call advances the cursor; once exhausted, the last entry is
/// returned indefinitely. `None` represents "printer not found" so we
/// can drive the disappearance pathway.
struct ScriptedBackend {
    states: Vec<Option<Printer>>,
    cursor: AtomicUsize,
}

impl ScriptedBackend {
    fn new(states: Vec<Option<Printer>>) -> Self {
        assert!(
            !states.is_empty(),
            "ScriptedBackend needs at least one state"
        );
        Self {
            states,
            cursor: AtomicUsize::new(0),
        }
    }

    fn current(&self) -> Option<Printer> {
        let idx = self
            .cursor
            .fetch_add(1, Ordering::SeqCst)
            .min(self.states.len() - 1);
        self.states[idx].clone()
    }
}

#[async_trait]
impl PrinterBackend for ScriptedBackend {
    async fn new() -> crate::Result<Self> {
        unreachable!("ScriptedBackend is constructed directly in tests")
    }

    async fn list_printers(&self) -> crate::Result<Vec<Printer>> {
        Ok(self.current().into_iter().collect())
    }

    async fn find_printer(&self, name: &str) -> crate::Result<Option<Printer>> {
        Ok(self
            .current()
            .filter(|p| p.name().eq_ignore_ascii_case(name)))
    }
}

fn monitor_with(backend: ScriptedBackend) -> PrinterMonitor {
    PrinterMonitor {
        backend: Arc::from(Box::new(backend) as Box<dyn PrinterBackend>),
    }
}

/// Runs `monitor_printer_changes` against the given backend until the
/// run window elapses, then cancels and returns the collected changes.
async fn collect_changes(
    backend: ScriptedBackend,
    printer_name: &'static str,
) -> Vec<PrinterChanges> {
    let monitor = monitor_with(backend);
    let captured: Arc<StdMutex<Vec<PrinterChanges>>> = Arc::new(StdMutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer_changes(
                printer_name,
                TEST_POLL_INTERVAL_MS,
                Some(cancel_clone),
                move |changes| {
                    captured_clone.lock().unwrap().push(changes.clone());
                },
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(TEST_RUN_DURATION_MS)).await;
    cancel.cancel();
    let _ = timeout(TEST_SHUTDOWN_TIMEOUT, handle).await;

    let lock = captured.lock().unwrap();
    lock.clone()
}

fn make_printer(name: &str, status: PrinterStatus, error: ErrorState, offline: bool) -> Printer {
    Printer::new(name.to_string(), status, error, offline, false)
}

#[tokio::test]
async fn callback_does_not_fire_on_initial_capture_only() {
    let printer = make_printer("Obs", PrinterStatus::Idle, ErrorState::NoError, false);
    // Same state forever - monitor should observe the initial capture
    // silently and then never fire (state never changes).
    let backend = ScriptedBackend::new(vec![Some(printer.clone()); 8]);

    let changes = collect_changes(backend, "Obs").await;
    assert!(
        changes.is_empty(),
        "expected no callbacks for stable printer state, got {:?}",
        changes
    );
}

#[tokio::test]
async fn callback_fires_on_real_status_change() {
    let idle = make_printer("Obs", PrinterStatus::Idle, ErrorState::NoError, false);
    let printing = make_printer("Obs", PrinterStatus::Printing, ErrorState::NoError, false);
    let backend = ScriptedBackend::new(vec![
        Some(idle.clone()),
        Some(idle),
        Some(printing.clone()),
        Some(printing),
    ]);

    let changes = collect_changes(backend, "Obs").await;
    assert!(
        changes.iter().any(|c| c.has_property_change("Status")),
        "expected Status change, got {:?}",
        changes
    );
}

#[tokio::test]
async fn callback_fires_on_disappearance() {
    // Present -> absent. Library synthesises IsOffline:false→true.
    let present = make_printer("Obs", PrinterStatus::Idle, ErrorState::NoError, false);
    let backend =
        ScriptedBackend::new(vec![Some(present.clone()), Some(present), None, None, None]);

    let changes = collect_changes(backend, "Obs").await;
    let saw_disappearance = changes.iter().any(|c| {
        c.changes.iter().any(|change| {
            matches!(
                change,
                PropertyChange::IsOffline {
                    old: false,
                    new: true,
                }
            )
        })
    });
    assert!(
        saw_disappearance,
        "expected IsOffline:false→true on disappearance, got {:?}",
        changes
    );
}

#[tokio::test]
async fn callback_fires_on_reappearance() {
    // Present -> absent -> present again. Library compares the
    // reappearance snapshot against its synthetic "missing" baseline
    // and reports IsOffline:true→false.
    let present = make_printer("Obs", PrinterStatus::Idle, ErrorState::NoError, false);
    let backend = ScriptedBackend::new(vec![
        Some(present.clone()),
        Some(present.clone()),
        None,
        None,
        Some(present.clone()),
        Some(present),
    ]);

    let changes = collect_changes(backend, "Obs").await;
    let saw_reappearance = changes.iter().any(|c| {
        c.changes.iter().any(|change| {
            matches!(
                change,
                PropertyChange::IsOffline {
                    old: true,
                    new: false,
                }
            )
        })
    });
    assert!(
        saw_reappearance,
        "expected IsOffline:true→false on reappearance, got {:?}",
        changes
    );
}

#[tokio::test]
async fn monitor_property_filters_to_selected_property() {
    // Both Status and ErrorState change. Subscribing to IsOffline only
    // must produce zero callbacks; subscribing to Status must surface
    // the Status change exactly once.
    let a = make_printer("Obs", PrinterStatus::Idle, ErrorState::NoError, false);
    let b = make_printer("Obs", PrinterStatus::Printing, ErrorState::Jammed, false);

    // Two backends because each test owns its scripted sequence.
    let backend_offline = ScriptedBackend::new(vec![
        Some(a.clone()),
        Some(a.clone()),
        Some(b.clone()),
        Some(b.clone()),
    ]);
    let monitor_offline = monitor_with(backend_offline);
    let offline_hits: Arc<StdMutex<usize>> = Arc::new(StdMutex::new(0));
    let offline_hits_clone = Arc::clone(&offline_hits);
    let cancel_offline = CancellationToken::new();
    let cancel_offline_clone = cancel_offline.clone();
    let h1 = tokio::spawn(async move {
        monitor_offline
            .monitor_property(
                "Obs",
                MonitorableProperty::IsOffline,
                TEST_POLL_INTERVAL_MS,
                Some(cancel_offline_clone),
                move |_| {
                    *offline_hits_clone.lock().unwrap() += 1;
                },
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(TEST_RUN_DURATION_MS)).await;
    cancel_offline.cancel();
    let _ = timeout(TEST_SHUTDOWN_TIMEOUT, h1).await;
    assert_eq!(
        *offline_hits.lock().unwrap(),
        0,
        "IsOffline subscriber must not fire for Status/ErrorState-only changes"
    );

    let backend_status =
        ScriptedBackend::new(vec![Some(a.clone()), Some(a), Some(b.clone()), Some(b)]);
    let monitor_status = monitor_with(backend_status);
    let status_hits: Arc<StdMutex<usize>> = Arc::new(StdMutex::new(0));
    let status_hits_clone = Arc::clone(&status_hits);
    let cancel_status = CancellationToken::new();
    let cancel_status_clone = cancel_status.clone();
    let h2 = tokio::spawn(async move {
        monitor_status
            .monitor_property(
                "Obs",
                MonitorableProperty::Status,
                TEST_POLL_INTERVAL_MS,
                Some(cancel_status_clone),
                move |_| {
                    *status_hits_clone.lock().unwrap() += 1;
                },
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(TEST_RUN_DURATION_MS)).await;
    cancel_status.cancel();
    let _ = timeout(TEST_SHUTDOWN_TIMEOUT, h2).await;
    assert!(
        *status_hits.lock().unwrap() >= 1,
        "Status subscriber must fire at least once when Status changes"
    );
}

#[tokio::test]
async fn monitor_printer_changes_initial_state_is_silent() {
    // Even when the very first observation is `Some(printer)`, the
    // callback must not fire for that initial capture - only later
    // diffs are reported.
    let printer = make_printer("Obs", PrinterStatus::Idle, ErrorState::NoError, false);
    let backend = ScriptedBackend::new(vec![Some(printer); 6]);
    let monitor = monitor_with(backend);
    let captured: Arc<StdMutex<Vec<PrinterChanges>>> = Arc::new(StdMutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        monitor
            .monitor_printer_changes("Obs", TEST_POLL_INTERVAL_MS, Some(cancel_clone), move |c| {
                captured_clone.lock().unwrap().push(c.clone())
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(TEST_RUN_DURATION_MS)).await;
    cancel.cancel();
    let _ = timeout(TEST_SHUTDOWN_TIMEOUT, handle).await;

    assert!(
        captured.lock().unwrap().is_empty(),
        "initial state must be captured silently"
    );
}
