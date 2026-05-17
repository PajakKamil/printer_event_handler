//! Event-driven monitoring backed by WMI `__InstanceModificationEvent`.
//!
//! Available only on Windows when the `events` cargo feature is enabled.
//! Wraps `wmi::WMIConnection::raw_notification` to subscribe to printer
//! modification events and forwards them as [`PrinterChanges`] through an
//! `mpsc` channel for the [`super::builder::MonitorBuilder`] stream API.
//!
//! On other platforms or without the `events` feature, [`event_changes_stream`]
//! is absent - the builder transparently falls back to polling.

#![cfg(all(windows, feature = "events"))]

use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::logging::{pe_error as error, pe_info as info, pe_warn as warn};
use crate::printer::Win32Printer;
use crate::{Printer, PrinterChanges};

/// WMI envelope around an `__InstanceModificationEvent` whose `TargetInstance`
/// is a `Win32_Printer`. We don't need `PreviousInstance` because the library
/// already maintains its own diff baseline.
#[derive(Deserialize, Debug)]
#[serde(rename = "__InstanceModificationEvent")]
#[serde(rename_all = "PascalCase")]
struct PrinterModificationEvent {
    target_instance: Win32Printer,
}

/// WQL polling interval (seconds) used in the `WITHIN` clause. WMI internally
/// polls the underlying class at this cadence; lower values give faster
/// reaction but more WMI load. One second matches what most Windows tools
/// (Performance Monitor, etc.) use for WMI subscriptions.
const EVENT_WITHIN_SECONDS: u32 = 1;

/// Spawns the WMI notification subscription on a tokio blocking thread and
/// forwards each filtered [`PrinterChanges`] into `tx`.
///
/// Cancellation is handled via the [`CancellationToken`] check between events
/// and by receiver-drop detection - worst-case responsiveness is the `WITHIN`
/// interval (~1 second).
///
/// On WMI failure (admin missing, service down, malformed query) a warning is
/// logged and the spawn exits without yielding events. The caller is
/// responsible for the channel ownership; this function returns immediately
/// after spawning the background work.
pub(super) fn spawn_event_subscription(
    tx: UnboundedSender<PrinterChanges>,
    printer_name: String,
    cancel_token: Option<CancellationToken>,
) {
    tokio::task::spawn_blocking(move || {
        let conn = match wmi::WMIConnection::new() {
            Ok(conn) => conn,
            Err(e) => {
                warn!(
                    "Event subscription disabled - failed to open WMI connection: {}",
                    e
                );
                return;
            }
        };

        let query = format!(
            "SELECT * FROM __InstanceModificationEvent WITHIN {} WHERE TargetInstance ISA 'Win32_Printer'",
            EVENT_WITHIN_SECONDS
        );
        let iter = match conn.raw_notification::<PrinterModificationEvent>(query) {
            Ok(iter) => iter,
            Err(e) => {
                warn!(
                    "Event subscription disabled - ExecNotificationQuery failed: {}",
                    e
                );
                return;
            }
        };

        info!(
            "WMI event subscription active for printer '{}'",
            printer_name
        );

        // Maintain our own diff baseline so the stream surface matches the
        // polling path (PrinterChanges, not raw Win32Printer snapshots).
        let mut previous: Option<Printer> = None;

        for event_result in iter {
            // Cooperative cancellation between events.
            if let Some(ref token) = cancel_token
                && token.is_cancelled()
            {
                info!(
                    "Event subscription for '{}' cancelled",
                    printer_name
                );
                return;
            }
            if tx.is_closed() {
                return;
            }

            let event = match event_result {
                Ok(event) => event,
                Err(e) => {
                    error!("WMI notification iterator failure: {}", e);
                    return;
                }
            };

            let current: Printer = event.target_instance.into();
            if !current.name().eq_ignore_ascii_case(&printer_name) {
                continue; // Different printer mutated; ignore.
            }

            if let Some(ref prev) = previous {
                let changes = prev.compare_with(&current);
                if changes.has_changes() && tx.send(changes).is_err() {
                    return;
                }
            }
            previous = Some(current);
        }
    });
}
