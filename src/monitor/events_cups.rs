//! Event-driven monitoring backed by CUPS's D-Bus signals.
//!
//! Available on Linux/macOS when the `events` cargo feature is enabled.
//! Subscribes to signals emitted by `cupsd`'s built-in D-Bus notifier
//! (interface `org.cups.cupsd.Notifier`) and forwards each printer state
//! transition as a [`PrinterChanges`] through an `mpsc` channel for the
//! [`super::builder::MonitorBuilder`] stream API.
//!
//! CUPS D-Bus signals only carry the event type (printer-state-changed,
//! printer-stopped, etc.) - not the post-change printer snapshot. So on
//! each signal we re-query the backend and diff the result against our
//! own baseline, mirroring how the Windows WMI path operates. This keeps
//! the change payload identical between the two event backends and the
//! pure polling fallback.
//!
//! If the system D-Bus connection or the match rule fails (cupsd built
//! without `--enable-dbus`, broker unavailable, etc.), the spawn logs a
//! warning and exits - callers transparently degrade to polling because
//! the same stream is also fed by the polling task in [`super::stream`].

#![cfg(all(unix, feature = "events"))]

use futures_util::stream::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use zbus::message::Type as MessageType;
use zbus::{Connection, MatchRule, MessageStream};

use super::PrinterMonitor;
use crate::logging::{pe_error as error, pe_info as info, pe_warn as warn};
use crate::{Printer, PrinterChanges};

/// D-Bus interface name `cupsd` publishes printer/job state signals on
/// (`org.cups.cupsd.Notifier`). Documented in the CUPS Programming Manual.
const CUPS_NOTIFIER_INTERFACE: &str = "org.cups.cupsd.Notifier";

/// Spawns the CUPS D-Bus notifier subscription on the tokio runtime and
/// forwards each filtered [`PrinterChanges`] into `tx`.
///
/// Lifecycle:
/// - Cancellation: between events via [`CancellationToken::is_cancelled`]
///   and as a `tokio::select!` arm during the `stream.next().await` so
///   responsiveness doesn't depend on signal cadence.
/// - Receiver-drop: detected via [`UnboundedSender::is_closed`] and a
///   `send().is_err()` short-circuit.
/// - D-Bus failure: logged at `warn!` and the spawn exits; callers should
///   have a polling fallback configured (which the higher-level
///   [`super::stream`] module wires up automatically when the event
///   subscription terminates).
pub(super) fn spawn_cups_subscription(
    tx: UnboundedSender<PrinterChanges>,
    printer_name: String,
    cancel_token: Option<CancellationToken>,
    monitor: PrinterMonitor,
) {
    tokio::spawn(async move {
        let conn = match Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "CUPS event subscription disabled - failed to open system D-Bus connection: {}",
                    e
                );
                return;
            }
        };

        // Filter at the broker so we only receive `cupsd` signals. The
        // `interface()` setter on the builder accepts an `&str` and returns
        // a `Result` (the string is validated for syntactic correctness).
        let rule = match MatchRule::builder()
            .msg_type(MessageType::Signal)
            .interface(CUPS_NOTIFIER_INTERFACE)
        {
            Ok(b) => b.build(),
            Err(e) => {
                warn!(
                    "CUPS event subscription disabled - invalid interface name: {}",
                    e
                );
                return;
            }
        };

        let mut stream = match MessageStream::for_match_rule(rule, &conn, None).await {
            Ok(s) => s,
            Err(e) => {
                warn!("CUPS event subscription disabled - AddMatch failed: {}", e);
                return;
            }
        };

        info!(
            "CUPS D-Bus event subscription active for printer '{}'",
            printer_name
        );

        let mut previous: Option<Printer> = None;

        loop {
            if let Some(ref token) = cancel_token
                && token.is_cancelled()
            {
                info!("CUPS event subscription for '{}' cancelled", printer_name);
                return;
            }
            if tx.is_closed() {
                return;
            }

            // Wait for the next D-Bus signal, with a cancellation arm so we
            // stop promptly when the token fires rather than after the
            // following signal arrives.
            let next = if let Some(ref token) = cancel_token {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("CUPS event subscription for '{}' cancelled", printer_name);
                        return;
                    }
                    next = stream.next() => next,
                }
            } else {
                stream.next().await
            };

            let _msg = match next {
                Some(Ok(m)) => m,
                Some(Err(e)) => {
                    error!("CUPS D-Bus stream error: {}", e);
                    return;
                }
                None => return,
            };

            // CUPS signals don't carry the post-change snapshot - re-query
            // and diff. We deliberately ignore signal payloads beyond their
            // arrival as a wake-up: the signal set differs across cupsd
            // versions, and a re-query gives us a stable, version-agnostic
            // view of the full printer state.
            let current = match monitor.backend.find_printer(&printer_name).await {
                Ok(Some(p)) => p,
                Ok(None) => continue,
                Err(e) => {
                    warn!(
                        "Failed to re-query printer '{}' after CUPS event: {}",
                        printer_name, e
                    );
                    continue;
                }
            };

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
