use crate::backend::{PrinterBackend, create_backend};
use crate::{Printer, PrinterChanges, Result};
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;

/// How many consecutive transient backend errors a monitor loop tolerates
/// before propagating the failure to the caller. A successful poll resets the
/// counter, so this only fires for sustained outages (e.g. WMI service down,
/// CUPS unreachable) - a single WMI hiccup no longer kills monitoring.
const MAX_CONSECUTIVE_MONITOR_ERRORS: u32 = 5;

/// Per-printer state carried across polls inside `monitor_multiple_printers`.
///
/// `snapshot` is the next-comparison baseline. After a fresh disappearance it
/// is replaced with a synthetic "missing" snapshot (Offline / UnknownError /
/// is_offline=true) so the reappearance comparison surfaces the
/// `IsOffline: true -> false` delta plus any other property differences (B4).
/// `was_present_last_poll` distinguishes a fresh disappearance from continued
/// absence, ensuring the disappearance callback fires exactly once per gap.
#[derive(Debug, Clone)]
struct PresenceTracker {
    snapshot: Option<Printer>,
    was_present_last_poll: bool,
}

impl PresenceTracker {
    fn new() -> Self {
        Self {
            snapshot: None,
            was_present_last_poll: false,
        }
    }
}

/// Enum representing all available printer properties that can be monitored.
///
/// This enum provides type-safe access to all printer properties that can be
/// monitored for changes, replacing string-based property names with a
/// strongly-typed interface.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonitorableProperty {
    /// Printer name changes
    Name,
    /// PrinterStatus enum changes (recommended current status)
    Status,
    /// PrinterState enum changes (legacy Windows state)
    State,
    /// ErrorState enum changes
    ErrorState,
    /// Online/offline status changes
    IsOffline,
    /// Default printer designation changes
    IsDefault,
    /// Raw PrinterStatus code changes (1-7)
    PrinterStatusCode,
    /// Raw PrinterState code changes (.NET flags)
    PrinterStateCode,
    /// Raw DetectedErrorState code changes (0-11)
    DetectedErrorStateCode,
    /// Raw ExtendedDetectedErrorState code changes
    ExtendedDetectedErrorStateCode,
    /// Raw ExtendedPrinterStatus code changes
    ExtendedPrinterStatusCode,
    /// WMI Status property changes ("OK", "Error", etc.)
    WmiStatus,
}

impl MonitorableProperty {
    /// Returns the string representation of the property name.
    ///
    /// This matches the property names used in the PropertyChange enum.
    pub fn as_str(&self) -> &'static str {
        match self {
            MonitorableProperty::Name => "Name",
            MonitorableProperty::Status => "Status",
            MonitorableProperty::State => "State",
            MonitorableProperty::ErrorState => "ErrorState",
            MonitorableProperty::IsOffline => "IsOffline",
            MonitorableProperty::IsDefault => "IsDefault",
            MonitorableProperty::PrinterStatusCode => "PrinterStatusCode",
            MonitorableProperty::PrinterStateCode => "PrinterStateCode",
            MonitorableProperty::DetectedErrorStateCode => "DetectedErrorStateCode",
            MonitorableProperty::ExtendedDetectedErrorStateCode => "ExtendedDetectedErrorStateCode",
            MonitorableProperty::ExtendedPrinterStatusCode => "ExtendedPrinterStatusCode",
            MonitorableProperty::WmiStatus => "WmiStatus",
        }
    }

    /// Returns a human-readable description of what this property represents.
    pub fn description(&self) -> &'static str {
        match self {
            MonitorableProperty::Name => "Printer name",
            MonitorableProperty::Status => "Current printer status (recommended)",
            MonitorableProperty::State => "Printer state (legacy Windows property)",
            MonitorableProperty::ErrorState => "Current error condition",
            MonitorableProperty::IsOffline => "Online/offline status",
            MonitorableProperty::IsDefault => "Default printer designation",
            MonitorableProperty::PrinterStatusCode => "Raw printer status code (1-7)",
            MonitorableProperty::PrinterStateCode => "Raw printer state code (.NET flags)",
            MonitorableProperty::DetectedErrorStateCode => "Raw detected error state code (0-11)",
            MonitorableProperty::ExtendedDetectedErrorStateCode => "Extended error state code",
            MonitorableProperty::ExtendedPrinterStatusCode => "Extended printer status code",
            MonitorableProperty::WmiStatus => "WMI status property",
        }
    }

    /// Returns all available properties that can be monitored.
    ///
    /// Returns a `&'static` slice so callers don't pay for an allocation just
    /// to enumerate the list. Use `.iter()` / `.contains(...)` / `.len()` as
    /// you would with a `Vec` - all the slice methods are available.
    pub fn all() -> &'static [MonitorableProperty] {
        &[
            MonitorableProperty::Name,
            MonitorableProperty::Status,
            MonitorableProperty::State,
            MonitorableProperty::ErrorState,
            MonitorableProperty::IsOffline,
            MonitorableProperty::IsDefault,
            MonitorableProperty::PrinterStatusCode,
            MonitorableProperty::PrinterStateCode,
            MonitorableProperty::DetectedErrorStateCode,
            MonitorableProperty::ExtendedDetectedErrorStateCode,
            MonitorableProperty::ExtendedPrinterStatusCode,
            MonitorableProperty::WmiStatus,
        ]
    }
}

/// Printer monitoring and querying functionality
///
/// `PrinterMonitor` is cheaply cloneable (uses `Arc` internally) and can be
/// shared across tasks without creating multiple backend connections.
#[derive(Clone)]
pub struct PrinterMonitor {
    backend: Arc<dyn PrinterBackend>,
}

impl PrinterMonitor {
    /// Creates a new PrinterMonitor instance with the appropriate platform backend.
    ///
    /// This function automatically selects and initializes the correct backend
    /// for the current platform (WMI for Windows, CUPS for Linux).
    ///
    /// # Returns
    /// * `Result<Self>` - A new PrinterMonitor instance or an error if initialization fails
    ///
    /// # Errors
    /// * `PrinterError::PlatformNotSupported` - If the current platform is not supported
    /// * `PrinterError::WmiError` - If WMI initialization fails on Windows
    /// * `PrinterError::CupsError` - If CUPS initialization fails on Linux
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    /// }
    /// ```
    pub async fn new() -> Result<Self> {
        info!("Initializing printer monitor...");
        let backend = create_backend().await?;
        Ok(Self {
            backend: Arc::from(backend),
        })
    }

    /// Retrieves a list of all printers available on the system.
    ///
    /// This method queries the platform-specific printer service to get
    /// information about all installed and available printers.
    ///
    /// # Returns
    /// * `Result<Vec<Printer>>` - A vector of all printers found on the system
    ///
    /// # Errors
    /// * `PrinterError::WmiError` - If the WMI query fails on Windows
    /// * `PrinterError::CupsError` - If the CUPS query fails on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let printers = monitor.list_printers().await.unwrap();
    ///     
    ///     for printer in printers {
    ///         println!("{}: {}", printer.name(), printer.status_description());
    ///     }
    /// }
    /// ```
    pub async fn list_printers(&self) -> Result<Vec<Printer>> {
        self.backend.list_printers().await
    }

    /// Searches for a specific printer by name using case-insensitive matching.
    ///
    /// This method searches through all available printers to find one with
    /// a name that matches the provided string (case-insensitive).
    ///
    /// # Arguments
    /// * `name` - The name of the printer to search for
    ///
    /// # Returns
    /// * `Result<Option<Printer>>` - The found printer or None if not found
    ///
    /// # Errors
    /// * `PrinterError::WmiError` - If the WMI query fails on Windows
    /// * `PrinterError::CupsError` - If the CUPS query fails on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     
    ///     if let Some(printer) = monitor.find_printer("HP LaserJet").await.unwrap() {
    ///         println!("Found printer: {}", printer.name());
    ///     }
    /// }
    /// ```
    pub async fn find_printer(&self, name: &str) -> Result<Option<Printer>> {
        self.backend.find_printer(name).await
    }

    /// Continuously monitors a specific printer for status changes.
    ///
    /// This function runs indefinitely, polling the specified printer every `interval_ms`
    /// milliseconds and calling the provided callback function whenever the printer's status changes.
    /// The callback receives both the current printer state and the previous state (if any).
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `callback` - Function called when printer status changes, receives (current, previous)
    ///
    /// # Returns
    /// * `Result<()>` - Never returns Ok normally (runs indefinitely), only Err on failure
    ///
    /// # Errors
    /// * `PrinterError::PrinterNotFound` - If the specified printer is not found initially
    /// * `PrinterError::WmiError` - If WMI queries fail on Windows
    /// * `PrinterError::CupsError` - If CUPS queries fail on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Behavior
    /// - If the printer disappears during monitoring, the callback is called with a synthetic
    ///   "unknown" status to indicate the printer is no longer available
    /// - The first check always triggers the callback to provide the initial status
    /// - Subsequent calls only trigger the callback if the status actually changes
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     monitor.monitor_printer("HP LaserJet", 30000, Some(cancel_token.clone()), |current, previous| {
    ///         if let Some(prev) = previous {
    ///             if prev != current {
    ///                 println!("Status changed: {} -> {}",
    ///                     prev.status_description(),
    ///                     current.status_description());
    ///             }
    ///         } else {
    ///             println!("Initial status: {}", current.status_description());
    ///         }
    ///     }).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_printer<F>(
        &self,
        printer_name: &str,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&Printer, Option<&Printer>) + Send,
    {
        info!("Starting printer monitoring service for: {}", printer_name);

        let mut previous_printer: Option<Printer> = None;
        let mut consecutive_errors: u32 = 0;

        loop {
            // Check for cancellation
            if let Some(ref token) = cancel_token
                && token.is_cancelled()
            {
                info!("Printer monitoring for '{}' cancelled", printer_name);
                return Ok(());
            }

            match self.find_printer(printer_name).await {
                Ok(Some(current_printer)) => {
                    consecutive_errors = 0;
                    info!("Checking printer: {}", current_printer.name());
                    let has_changed = previous_printer
                        .as_ref()
                        .map(|prev| prev != &current_printer)
                        .unwrap_or(true);

                    if has_changed {
                        callback(&current_printer, previous_printer.as_ref());
                        info!(
                            "Printer '{}' - Status: {}, Error: {}",
                            printer_name,
                            current_printer.status_description(),
                            current_printer.error_description()
                        );
                        previous_printer = Some(current_printer);
                    } else {
                        info!("Printer '{}' status unchanged", printer_name);
                    }
                }
                Ok(None) => {
                    consecutive_errors = 0;
                    warn!("Printer '{}' not found", printer_name);
                    if previous_printer.is_some() {
                        // Printer was previously found but now missing
                        callback(
                            &Printer::new(
                                printer_name.to_string(),
                                crate::PrinterStatus::StatusUnknown,
                                crate::ErrorState::UnknownError,
                                true,
                                false,
                            ),
                            previous_printer.as_ref(),
                        );
                        previous_printer = None;
                    }
                }
                Err(e) => {
                    consecutive_errors += 1;
                    if consecutive_errors >= MAX_CONSECUTIVE_MONITOR_ERRORS {
                        error!(
                            "Printer '{}' monitoring failed after {} consecutive errors, last: {}",
                            printer_name, consecutive_errors, e
                        );
                        return Err(e);
                    }
                    warn!(
                        "Transient error checking printer '{}' (attempt {}/{}), will retry: {}",
                        printer_name, consecutive_errors, MAX_CONSECUTIVE_MONITOR_ERRORS, e
                    );
                }
            }

            // Sleep with cancellation support
            if let Some(ref token) = cancel_token {
                tokio::select! {
                    _ = sleep(Duration::from_millis(interval_ms)) => {},
                    _ = token.cancelled() => {
                        info!("Printer monitoring for '{}' cancelled during sleep", printer_name);
                        return Ok(());
                    }
                }
            } else {
                sleep(Duration::from_millis(interval_ms)).await;
            }
        }
    }

    /// Retrieves a comprehensive summary of all printers and their current states.
    ///
    /// This method provides a convenient way to get an overview of all printers
    /// in a structured format, useful for status dashboards or reports.
    ///
    /// # Returns
    /// * `Result<HashMap<String, PrinterSummary>>` - Map of printer names to their summaries
    ///
    /// # Errors
    /// * `PrinterError::WmiError` - If the WMI query fails on Windows
    /// * `PrinterError::CupsError` - If the CUPS query fails on Linux
    /// * `PrinterError::IoError` - If there are system I/O issues
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let summary = monitor.printer_summary().await.unwrap();
    ///     
    ///     for (name, info) in summary {
    ///         println!("{}: {} ({})", name, info.status,
    ///             if info.has_error { "ERROR" } else { "OK" });
    ///     }
    /// }
    /// ```
    pub async fn printer_summary(&self) -> Result<HashMap<String, PrinterSummary>> {
        let printers = self.list_printers().await?;
        let mut summary = HashMap::new();

        for printer in printers {
            summary.insert(
                printer.name().to_string(),
                PrinterSummary {
                    status: printer.status().clone(),
                    error_state: printer.error_state().clone(),
                    is_offline: printer.is_offline(),
                    is_default: printer.is_default(),
                    has_error: printer.has_error(),
                },
            );
        }

        Ok(summary)
    }

    /// Monitors a printer with detailed property change detection.
    ///
    /// This enhanced monitoring method provides detailed information about exactly which
    /// properties changed between checks, enabling fine-grained monitoring and alerting.
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `cancel_token` - Optional cancellation token for graceful shutdown
    /// * `callback` - Function called when properties change, receives PrinterChanges
    ///
    /// # Returns
    /// * `Result<()>` - Returns Ok when cancelled, or Err on failure
    ///
    /// # Behavior
    /// - The callback is **NOT** called on the initial state capture
    /// - Only actual property changes trigger the callback
    /// - Changes are always non-empty when the callback is invoked
    /// - The initial state is captured silently for comparison with future states
    /// - Disappearance fires a synthetic `IsOffline: false -> true` change
    /// - Reappearance fires a change set comparing the missing-state snapshot
    ///   with the current printer (typically includes `IsOffline: true -> false`)
    /// - Monitoring stops gracefully when the cancellation token is cancelled
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     // Clone the token for the monitoring task
    ///     let token_clone = cancel_token.clone();
    ///     let handle = tokio::spawn(async move {
    ///         monitor.monitor_printer_changes("HP LaserJet", 30000, Some(token_clone), |changes| {
    ///             println!("Detected {} changes:", changes.change_count());
    ///             for change in &changes.changes {
    ///                 println!("  - {}", change.description());
    ///             }
    ///         }).await
    ///     });
    ///
    ///     // Later: cancel monitoring
    ///     cancel_token.cancel();
    ///     handle.await.unwrap().unwrap();
    /// }
    /// ```
    pub async fn monitor_printer_changes<F>(
        &self,
        printer_name: &str,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&PrinterChanges) + Send,
    {
        info!(
            "Starting detailed printer change monitoring for: {}",
            printer_name
        );

        // `previous_printer` is the snapshot the next poll compares against.
        // After a disappearance it's replaced with a synthetic "missing"
        // snapshot so reappearance surfaces the IsOffline:true->false delta and
        // any other properties that changed during the gap (B4).
        let mut previous_printer: Option<Printer> = None;
        let mut was_present: bool = false;
        let mut consecutive_errors: u32 = 0;

        loop {
            // Check for cancellation
            if let Some(ref token) = cancel_token
                && token.is_cancelled()
            {
                info!("Printer monitoring for '{}' cancelled", printer_name);
                return Ok(());
            }

            match self.find_printer(printer_name).await {
                Ok(Some(current_printer)) => {
                    consecutive_errors = 0;
                    if let Some(ref prev) = previous_printer {
                        let changes = prev.compare_with(&current_printer);
                        if changes.has_changes() {
                            info!(
                                "Printer '{}' - {} properties changed",
                                printer_name,
                                changes.change_count()
                            );
                            callback(&changes);
                        }
                    } else {
                        // Initial state - just capture, don't call callback
                        info!("Printer '{}' - Initial state captured", printer_name);
                    }
                    previous_printer = Some(current_printer);
                    was_present = true;
                }
                Ok(None) => {
                    consecutive_errors = 0;
                    warn!("Printer '{}' not found", printer_name);
                    if was_present {
                        if let Some(ref prev) = previous_printer {
                            // Fresh disappearance - synthesize an IsOffline change.
                            let mut changes = PrinterChanges::new(printer_name.to_string());
                            changes.changes.push(crate::PropertyChange::IsOffline {
                                old: prev.is_offline(),
                                new: true,
                            });
                            callback(&changes);
                        }
                        // Replace with a synthetic "missing" snapshot so the
                        // next successful poll surfaces the reappearance.
                        previous_printer = Some(Printer::new(
                            printer_name.to_string(),
                            crate::PrinterStatus::Offline,
                            crate::ErrorState::UnknownError,
                            true,
                            false,
                        ));
                    }
                    was_present = false;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    if consecutive_errors >= MAX_CONSECUTIVE_MONITOR_ERRORS {
                        error!(
                            "Printer '{}' monitoring failed after {} consecutive errors, last: {}",
                            printer_name, consecutive_errors, e
                        );
                        return Err(e);
                    }
                    warn!(
                        "Transient error checking printer '{}' (attempt {}/{}), will retry: {}",
                        printer_name, consecutive_errors, MAX_CONSECUTIVE_MONITOR_ERRORS, e
                    );
                }
            }

            // Sleep with cancellation support
            if let Some(ref token) = cancel_token {
                tokio::select! {
                    _ = sleep(Duration::from_millis(interval_ms)) => {},
                    _ = token.cancelled() => {
                        info!("Printer monitoring for '{}' cancelled during sleep", printer_name);
                        return Ok(());
                    }
                }
            } else {
                sleep(Duration::from_millis(interval_ms)).await;
            }
        }
    }

    /// Monitors a specific property of a printer for changes.
    ///
    /// This method allows monitoring just a single property, useful for alerting
    /// on specific conditions like offline status or error state changes.
    ///
    /// # Arguments
    /// * `printer_name` - The name of the printer to monitor
    /// * `property` - The specific property to watch using MonitorableProperty enum
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `cancel_token` - Optional cancellation token for graceful shutdown
    /// * `callback` - Function called when the property changes
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::{PrinterMonitor, MonitorableProperty};
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     monitor.monitor_property(
    ///         "HP LaserJet",
    ///         MonitorableProperty::IsOffline,
    ///         60000,
    ///         Some(cancel_token.clone()),
    ///         |change| {
    ///             println!("Offline status changed: {}", change.description());
    ///         }
    ///     ).await.unwrap();
    /// }
    /// ```
    pub async fn monitor_property<F>(
        &self,
        printer_name: &str,
        property: MonitorableProperty,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&crate::PropertyChange) + Send,
    {
        let property_name = property.as_str();
        info!(
            "Starting property '{}' monitoring for printer: {}",
            property_name, printer_name
        );

        self.monitor_printer_changes(printer_name, interval_ms, cancel_token, move |changes| {
            for change in &changes.changes {
                if change.property_name() == property_name {
                    callback(change);
                }
            }
        })
        .await
    }

    /// Monitors multiple printers concurrently and reports changes for any of them.
    ///
    /// This method allows monitoring several printers simultaneously, with a single
    /// callback that receives changes from any of the monitored printers.
    ///
    /// # Arguments
    /// * `printer_names` - List of printer names to monitor
    /// * `interval_ms` - Polling interval in milliseconds
    /// * `cancel_token` - Optional cancellation token for graceful shutdown
    /// * `callback` - Function called when any printer changes (NOT called on initial state)
    ///
    /// # Returns
    /// * `Result<()>` - Returns Ok when cancelled or all tasks complete, or Err on failure
    ///
    /// # Behavior
    /// - The callback is **NOT** called on the initial state capture for each printer
    /// - Only actual property changes trigger the callback
    /// - Disappearance fires a synthetic `IsOffline: false -> true` change once per gap
    /// - Reappearance fires a change set comparing the missing-state snapshot
    ///   with the current printer (typically includes `IsOffline: true -> false`)
    /// - Each printer is monitored in a separate task for true concurrent monitoring
    /// - Callbacks are called outside of internal locks to prevent contention with slow callbacks
    /// - The monitor is cloned (cheaply via Arc) for each task, sharing the same backend connection
    /// - Monitoring stops gracefully when the cancellation token is cancelled
    ///
    /// # Example
    /// ```rust,no_run
    /// use printer_event_handler::PrinterMonitor;
    /// use tokio_util::sync::CancellationToken;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let monitor = PrinterMonitor::new().await.unwrap();
    ///     let printers = vec!["HP LaserJet".to_string(), "Canon Printer".to_string()];
    ///     let cancel_token = CancellationToken::new();
    ///
    ///     let token_clone = cancel_token.clone();
    ///     let handle = tokio::spawn(async move {
    ///         monitor.monitor_multiple_printers(printers, 30000, Some(token_clone), |changes| {
    ///             println!("Printer '{}' changed: {}", changes.printer_name, changes.summary());
    ///         }).await
    ///     });
    ///
    ///     // Later: cancel monitoring
    ///     cancel_token.cancel();
    ///     handle.await.unwrap().unwrap();
    /// }
    /// ```
    pub async fn monitor_multiple_printers<F>(
        &self,
        printer_names: Vec<String>,
        interval_ms: u64,
        cancel_token: Option<CancellationToken>,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(&PrinterChanges) + Send + Sync + 'static,
    {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        info!(
            "Starting concurrent monitoring of {} printers",
            printer_names.len()
        );

        let callback = Arc::new(callback);
        // JoinSet (over Vec<JoinHandle>) so a single task failure can abort the
        // siblings via `abort_all()`, preventing the orphaned-poller leak that
        // happened when the caller passed no CancellationToken.
        let mut tasks: JoinSet<Result<()>> = JoinSet::new();
        let mut previous_states: HashMap<String, PresenceTracker> = HashMap::new();

        // Initialize per-printer trackers
        for name in &printer_names {
            previous_states.insert(name.clone(), PresenceTracker::new());
        }

        let previous_states = Arc::new(tokio::sync::Mutex::new(previous_states));

        for printer_name in printer_names {
            let callback_clone = callback.clone();
            let printer_name_clone = printer_name.clone();
            let previous_states_clone = previous_states.clone();
            let monitor_clone = self.clone(); // Cheap Arc clone - shares the same backend
            let cancel_token_clone = cancel_token.clone();

            // Clone the monitor (cheap Arc clone) for each task, sharing the same backend connection
            tasks.spawn(async move {
                let mut consecutive_errors: u32 = 0;

                loop {
                    // Check for cancellation
                    if let Some(ref token) = cancel_token_clone
                        && token.is_cancelled()
                    {
                        info!("Printer monitoring for '{}' cancelled", printer_name_clone);
                        return Ok(());
                    }

                    match monitor_clone.find_printer(&printer_name_clone).await {
                        Ok(Some(current_printer)) => {
                            consecutive_errors = 0;
                            // Acquire lock to check previous state and compute changes
                            let (changes_to_report, is_initial) = {
                                let mut states = previous_states_clone.lock().await;
                                let tracker = states
                                    .entry(printer_name_clone.clone())
                                    .or_insert_with(PresenceTracker::new);

                                let result = if let Some(ref prev) = tracker.snapshot {
                                    let changes = prev.compare_with(&current_printer);
                                    if changes.has_changes() {
                                        (Some(changes), false)
                                    } else {
                                        (None, false)
                                    }
                                } else {
                                    // Never seen this printer before - silent capture.
                                    (None, true)
                                };

                                tracker.snapshot = Some(current_printer);
                                tracker.was_present_last_poll = true;
                                result
                            };
                            // Lock is released here

                            // Call callback outside of lock to avoid contention
                            if let Some(changes) = changes_to_report {
                                info!(
                                    "Printer '{}' - {} properties changed",
                                    printer_name_clone,
                                    changes.change_count()
                                );
                                callback_clone(&changes);
                            } else if is_initial {
                                info!("Printer '{}' - Initial state captured", printer_name_clone);
                            }
                        }
                        Ok(None) => {
                            consecutive_errors = 0;
                            warn!("Printer '{}' not found", printer_name_clone);

                            // Acquire lock to handle disappearance / continued absence.
                            let changes_to_report = {
                                let mut states = previous_states_clone.lock().await;
                                let tracker = states
                                    .entry(printer_name_clone.clone())
                                    .or_insert_with(PresenceTracker::new);

                                let changes = if tracker.was_present_last_poll {
                                    // Fresh disappearance - synthesize IsOffline transition.
                                    tracker.snapshot.as_ref().map(|prev| {
                                        let mut changes =
                                            PrinterChanges::new(printer_name_clone.clone());
                                        changes.changes.push(crate::PropertyChange::IsOffline {
                                            old: prev.is_offline(),
                                            new: true,
                                        });
                                        changes
                                    })
                                } else {
                                    None
                                };

                                if tracker.was_present_last_poll {
                                    // Replace the snapshot with a synthetic "missing"
                                    // baseline so the next successful poll surfaces
                                    // the reappearance delta (B4).
                                    tracker.snapshot = Some(Printer::new(
                                        printer_name_clone.clone(),
                                        crate::PrinterStatus::Offline,
                                        crate::ErrorState::UnknownError,
                                        true,
                                        false,
                                    ));
                                }
                                tracker.was_present_last_poll = false;
                                changes
                            };
                            // Lock is released here

                            // Call callback outside of lock
                            if let Some(changes) = changes_to_report {
                                callback_clone(&changes);
                            }
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            if consecutive_errors >= MAX_CONSECUTIVE_MONITOR_ERRORS {
                                error!(
                                    "Printer '{}' monitoring failed after {} consecutive errors, last: {}",
                                    printer_name_clone, consecutive_errors, e
                                );
                                return Err(e);
                            }
                            warn!(
                                "Transient error checking printer '{}' (attempt {}/{}), will retry: {}",
                                printer_name_clone, consecutive_errors, MAX_CONSECUTIVE_MONITOR_ERRORS, e
                            );
                        }
                    }

                    // Sleep with cancellation support
                    if let Some(ref token) = cancel_token_clone {
                        tokio::select! {
                            _ = sleep(Duration::from_millis(interval_ms)) => {},
                            _ = token.cancelled() => {
                                info!("Printer monitoring for '{}' cancelled during sleep", printer_name_clone);
                                return Ok(());
                            }
                        }
                    } else {
                        sleep(Duration::from_millis(interval_ms)).await;
                    }
                }
            });
        }

        // Process tasks as they finish; on the first real error, abort siblings
        // and return. JoinError::is_cancelled() is expected after abort_all() so
        // those drain silently.
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {
                    info!("Monitoring task completed successfully");
                }
                Ok(Err(e)) => {
                    error!("Monitoring task failed: {}", e);
                    tasks.abort_all();
                    return Err(e);
                }
                Err(je) if je.is_cancelled() => {
                    // Sibling aborted via abort_all() above - expected, ignore.
                }
                Err(je) => {
                    tasks.abort_all();
                    // Distinguish panic from other join errors and pull the
                    // payload string out of a panic so the caller can see what
                    // actually went wrong. JoinError's Display only includes
                    // "task panicked" / "task was cancelled" without the
                    // payload, which is what F6 was about.
                    let detail = if je.is_panic() {
                        match je.try_into_panic() {
                            Ok(payload) => {
                                let msg = payload
                                    .downcast_ref::<&'static str>()
                                    .map(|s| (*s).to_string())
                                    .or_else(|| payload.downcast_ref::<String>().cloned())
                                    .unwrap_or_else(|| "non-string panic payload".to_string());
                                format!("monitoring task panicked: {}", msg)
                            }
                            Err(je) => format!("monitoring task panicked: {}", je),
                        }
                    } else {
                        format!("monitoring task join failed: {}", je)
                    };
                    error!("{}", detail);
                    return Err(crate::PrinterError::Other(detail));
                }
            }
        }

        Ok(())
    }
}

/// Summary information about a printer's current state.
///
/// This struct provides a snapshot of a printer's essential status information
/// in a convenient format for reporting and monitoring applications.
#[derive(Debug, Clone)]
pub struct PrinterSummary {
    /// Current operational status of the printer
    pub status: crate::PrinterStatus,
    /// Current error state of the printer
    pub error_state: crate::ErrorState,
    /// Whether the printer is currently offline
    pub is_offline: bool,
    /// Whether this is the system's default printer
    pub is_default: bool,
    /// Whether the printer currently has any error conditions
    pub has_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(windows)]
    async fn test_monitor_creation() {
        let result = PrinterMonitor::new().await;
        // This might fail in CI/test environments without proper WMI access
        // but it should at least compile and attempt the connection
        match result {
            Ok(_) => println!("Monitor created successfully"),
            Err(e) => println!("Expected error in test environment: {}", e),
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_monitor_unix_creation() {
        let result = PrinterMonitor::new().await;
        // On Unix/Linux, the monitor should be created successfully
        assert!(result.is_ok());
    }

    #[test]
    fn test_monitorable_property_as_str() {
        assert_eq!(MonitorableProperty::Name.as_str(), "Name");
        assert_eq!(MonitorableProperty::Status.as_str(), "Status");
        assert_eq!(MonitorableProperty::State.as_str(), "State");
        assert_eq!(MonitorableProperty::ErrorState.as_str(), "ErrorState");
        assert_eq!(MonitorableProperty::IsOffline.as_str(), "IsOffline");
        assert_eq!(MonitorableProperty::IsDefault.as_str(), "IsDefault");
        assert_eq!(
            MonitorableProperty::PrinterStatusCode.as_str(),
            "PrinterStatusCode"
        );
        assert_eq!(
            MonitorableProperty::PrinterStateCode.as_str(),
            "PrinterStateCode"
        );
        assert_eq!(
            MonitorableProperty::DetectedErrorStateCode.as_str(),
            "DetectedErrorStateCode"
        );
        assert_eq!(
            MonitorableProperty::ExtendedDetectedErrorStateCode.as_str(),
            "ExtendedDetectedErrorStateCode"
        );
        assert_eq!(
            MonitorableProperty::ExtendedPrinterStatusCode.as_str(),
            "ExtendedPrinterStatusCode"
        );
        assert_eq!(MonitorableProperty::WmiStatus.as_str(), "WmiStatus");
    }

    #[test]
    fn test_monitorable_property_description() {
        assert_eq!(MonitorableProperty::Name.description(), "Printer name");
        assert_eq!(
            MonitorableProperty::Status.description(),
            "Current printer status (recommended)"
        );
        assert_eq!(
            MonitorableProperty::State.description(),
            "Printer state (legacy Windows property)"
        );
        assert_eq!(
            MonitorableProperty::IsOffline.description(),
            "Online/offline status"
        );
    }

    #[test]
    fn test_monitorable_property_all() {
        let all = MonitorableProperty::all();
        assert_eq!(all.len(), 12);

        // Verify all variants are present
        assert!(all.contains(&MonitorableProperty::Name));
        assert!(all.contains(&MonitorableProperty::Status));
        assert!(all.contains(&MonitorableProperty::State));
        assert!(all.contains(&MonitorableProperty::ErrorState));
        assert!(all.contains(&MonitorableProperty::IsOffline));
        assert!(all.contains(&MonitorableProperty::IsDefault));
        assert!(all.contains(&MonitorableProperty::PrinterStatusCode));
        assert!(all.contains(&MonitorableProperty::PrinterStateCode));
        assert!(all.contains(&MonitorableProperty::DetectedErrorStateCode));
        assert!(all.contains(&MonitorableProperty::ExtendedDetectedErrorStateCode));
        assert!(all.contains(&MonitorableProperty::ExtendedPrinterStatusCode));
        assert!(all.contains(&MonitorableProperty::WmiStatus));
    }

    #[test]
    fn test_monitorable_property_equality() {
        assert_eq!(MonitorableProperty::Status, MonitorableProperty::Status);
        assert_ne!(MonitorableProperty::Status, MonitorableProperty::State);
    }

    #[test]
    fn test_printer_summary_structure() {
        use crate::{ErrorState, PrinterStatus};

        let summary = PrinterSummary {
            status: PrinterStatus::Idle,
            error_state: ErrorState::NoError,
            is_offline: false,
            is_default: true,
            has_error: false,
        };

        assert_eq!(summary.status, PrinterStatus::Idle);
        assert_eq!(summary.error_state, ErrorState::NoError);
        assert!(!summary.is_offline);
        assert!(summary.is_default);
        assert!(!summary.has_error);
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn test_list_printers_windows() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let printers = monitor.list_printers().await;
            // Either we get printers or an error, but it should return something
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers", printer_list.len());
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_list_printers_unix() {
        let monitor = PrinterMonitor::new().await;
        assert!(monitor.is_ok());

        if let Ok(monitor) = monitor {
            let printers = monitor.list_printers().await;
            // Should return either printers or an error, but not panic
            match printers {
                Ok(printer_list) => {
                    println!("Found {} printers", printer_list.len());
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_find_nonexistent_printer() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let result = monitor.find_printer("NonExistentPrinter_12345_ABCDE").await;
            match result {
                Ok(None) => {
                    // Expected: printer not found
                }
                Ok(Some(_)) => {
                    panic!("Unexpectedly found a printer with unlikely name");
                }
                Err(_) => {
                    // Also acceptable in test environments
                }
            }
        }
    }

    #[tokio::test]
    async fn test_printer_summary() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let summary = monitor.printer_summary().await;
            // Should return either a summary or an error, but not panic
            match summary {
                Ok(summary_map) => {
                    println!("Got summary for {} printers", summary_map.len());
                }
                Err(e) => {
                    println!("Expected error in test environment: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_cancellation_token() {
        use crate::CancellationToken;
        use std::time::Duration;
        use tokio::time::timeout;

        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            // Spawn a task that will be cancelled
            let handle = tokio::spawn(async move {
                monitor
                    .monitor_printer_changes(
                        "NonExistentPrinter_Test",
                        1000,
                        Some(cancel_clone),
                        |_changes| {
                            // This should never be called
                            panic!("Should not receive changes for nonexistent printer");
                        },
                    )
                    .await
            });

            // Wait a bit then cancel
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_token.cancel();

            // The task should complete quickly after cancellation
            let result = timeout(Duration::from_secs(2), handle).await;
            assert!(result.is_ok(), "Task should complete after cancellation");
            if let Ok(Ok(task_result)) = result {
                assert!(task_result.is_ok(), "Cancelled monitoring should return Ok");
            }
        }
    }

    #[tokio::test]
    async fn test_monitor_without_cancellation() {
        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            // Monitor without a cancellation token should still work
            let handle = tokio::spawn(async move {
                monitor
                    .monitor_printer_changes(
                        "NonExistentPrinter_Test",
                        5000,
                        None, // No cancellation token
                        |_changes| {
                            // This should never be called
                        },
                    )
                    .await
            });

            // Let it run a bit
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Abort the task since we have no cancellation token
            handle.abort();

            // Task should be aborted
            let result = handle.await;
            assert!(result.is_err(), "Task should be aborted");
        }
    }

    #[tokio::test]
    async fn test_multiple_printers_cancellation() {
        use crate::CancellationToken;
        use std::time::Duration;
        use tokio::time::timeout;

        let monitor = PrinterMonitor::new().await;
        if let Ok(monitor) = monitor {
            let cancel_token = CancellationToken::new();
            let cancel_clone = cancel_token.clone();

            let printer_names = vec!["Printer1".to_string(), "Printer2".to_string()];

            // Spawn monitoring task
            let handle = tokio::spawn(async move {
                monitor
                    .monitor_multiple_printers(
                        printer_names,
                        1000,
                        Some(cancel_clone),
                        |_changes| {
                            // Should not be called
                        },
                    )
                    .await
            });

            // Wait a bit then cancel
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_token.cancel();

            // Should complete quickly
            let result = timeout(Duration::from_secs(2), handle).await;
            assert!(
                result.is_ok(),
                "Multi-printer monitoring should complete after cancellation"
            );
        }
    }

    #[test]
    fn test_printer_monitor_is_clone() {
        // Test that PrinterMonitor implements Clone
        fn assert_clone<T: Clone>() {}
        assert_clone::<PrinterMonitor>();
    }
}
