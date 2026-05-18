# Examples

Walkthroughs for `printer_event_handler` 2.0. Each binary is self-contained -
pick the one closest to what you want to build.

## Building

The examples have their own `Cargo.toml` so the main library can stay slim.
They depend on the parent crate with the `events` feature enabled so
`events_demo` exercises the WMI / D-Bus subscription path rather than the
polling fallback.

```bash
# From the examples directory
cd examples
cargo run --bin basic_listing

# Or from the project root, via the manifest path
cargo run --manifest-path examples/Cargo.toml --bin basic_listing
```

## Recommended order

If you are new to the crate, the examples roughly progress from the
simplest API to the most advanced:

1. **`basic_listing`** - one-shot enumeration via `list_printers_cancellable`
   and `printer_summary_iter`.
2. **`monitor_changes`** - the `MonitorBuilder` fluent surface
   (`monitor(name).interval_ms(...).run_printer(...)`) for current+previous
   snapshot monitoring.
3. **`property_monitoring`** - `filter_property(...)` + `run_property` to
   watch a single field, plus the underlying `monitor_multiple_printers`
   call for fanning across queues.
4. **`streaming_changes`** - `run_changes_stream` / `run_property_stream`
   for code that prefers `tokio_stream::Stream` over a callback.
5. **`cancellation_token_example`** - `CancellationToken` plumbing through
   `MonitorBuilder::cancel_token` and across concurrent monitors.
6. **`error_handling`** - matches on `PrinterError` (including the
   `#[non_exhaustive]` wildcard arm), retry / fallback patterns, and the
   `Cancelled` / `TaskPanicked` variants new in 2.0.
7. **`async_patterns`** - `JoinSet`, `mpsc` / `ReceiverStream`, shared state
   with `RwLock`, plus the library's high-level
   `monitor_multiple_printers` for the same fan-out.
8. **`events_demo`** - `with_events(true)` for the WMI / D-Bus subscription
   path. Falls back to polling with a warn-log if the `events` feature is
   off.
9. **`jobs_listing`** - `list_jobs` over all printers or a single named
   queue. Showcases the Windows / lpstat / libcups differences in the
   populated [`Job`] fields.

## 2.0 changes worth knowing

If you are porting code from 1.x:

- `PrinterMonitor::list_printers` and `find_printer` were removed. Use
  `list_printers_cancellable(None)` and `find_printer_cancellable(name, None)`
  - pass `Some(token)` to abort the query.
- Every public enum is `#[non_exhaustive]` (`PrinterError`, `PrinterState`,
  `ErrorState`, `PrinterStatus`, `JobStatus`, `MonitorableProperty`,
  `PropertyChange`). Exhaustive `match`es need a wildcard arm - see
  `error_handling.rs`.
- The `monitor_printer*` direct methods still exist, but
  `monitor.monitor(name).interval_ms(...).run_*(...)` is the recommended
  fluent form. Examples 2-5 use it.
- `run_changes_stream` and `run_property_stream` now yield `Result<T>`
  instead of `T`. A terminal backend failure (sustained WMI/CUPS outage)
  arrives as the stream's final `Err` before it closes; clean shutdowns
  close the stream silently. See `streaming_changes.rs` / `events_demo.rs`
  for the `match item { Ok(...) => ..., Err(e) => ... }` shape.
- `PrinterBackend::list_jobs` is now required (no default impl). The
  `jobs_listing` example calls it via `PrinterMonitor::list_jobs`.
- New typed `PrinterError` variants: `Cancelled` (returned by the
  `*_cancellable` methods when the token wins the race) and
  `TaskPanicked { printer_name, panic_message }` (surfaced by
  `monitor_multiple_printers` so callers can match on the failing
  printer instead of parsing strings). Both demoed in `error_handling.rs`.

## Cargo features the examples touch

- `events` (enabled by default for the example workspace) - flips
  `with_events(true)` from "warn and poll" to a real WMI or CUPS D-Bus
  subscription. See `events_demo.rs`.
- `linux-libcups` (opt-in, requires `libcups2-dev` / `cups-devel`) - swaps
  the lpstat subprocess parser for direct libcups FFI. Affects which
  fields `jobs_listing` can populate. Build with
  `cargo build --manifest-path examples/Cargo.toml --features printer_event_handler/linux-libcups`.

## Platform notes

- **Windows**: every WMI property is populated and the event path uses
  `__InstanceModificationEvent`.
- **Linux (lpstat backend)**: basic status, no page counters or document
  name on jobs, polling-only unless `events` is enabled.
- **Linux (libcups backend)**: structured printer / job data, document
  title on jobs, polling fallback. Same `events` D-Bus path applies.
