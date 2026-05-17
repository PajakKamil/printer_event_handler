//! Print Jobs Listing Example
//!
//! Demonstrates `PrinterMonitor::list_jobs`, the 2.0 API for enumerating
//! print jobs across all printers or on a single named queue. Behind the
//! scenes:
//!
//! - **Windows**: pulls `Win32_PrintJob` rows via WMI. Fields include the
//!   raw `JobStatus` bitmask, total / printed page counts, document name,
//!   owner and submit time.
//! - **Linux (default lpstat backend)**: parses `lpstat -o`. Submit time
//!   and owner are filled; page counters and document name are `None`
//!   because `lpstat` doesn't expose them.
//! - **Linux (`linux-libcups` feature)**: queries libcups directly via
//!   `cupsGetJobs2()`. Document title is populated; page counters are
//!   `None` because the CUPS C API doesn't surface them.
//!
//! Both paths funnel into the same [`Job`] type, so the consuming code is
//! identical regardless of backend. Fields the active backend doesn't
//! populate stay `None`.
//!
//! Run with:
//! ```bash
//! cargo run --manifest-path examples/Cargo.toml --bin jobs_listing
//! cargo run --manifest-path examples/Cargo.toml --bin jobs_listing -- "Printer Name"
//! ```

use std::env;

use printer_event_handler::{Job, PrinterError, PrinterMonitor};

#[tokio::main]
async fn main() -> Result<(), PrinterError> {
    env_logger::init();

    println!("Printer Event Handler - Print Jobs Listing");
    println!("===========================================\n");

    let monitor = PrinterMonitor::new().await?;

    // Optional positional arg: scope the listing to a single printer.
    // Without it we ask the backend for jobs across every queue.
    let printer_arg = env::args().nth(1);
    let jobs = match printer_arg.as_deref() {
        Some(name) => {
            println!("Listing jobs queued for '{}'...\n", name);
            monitor.list_jobs(Some(name), None).await?
        }
        None => {
            println!("Listing jobs across all printers...\n");
            monitor.list_jobs(None, None).await?
        }
    };

    if jobs.is_empty() {
        println!("No jobs found.");
        print_platform_note();
        return Ok(());
    }

    println!("Found {} job(s):\n", jobs.len());
    for (i, job) in jobs.iter().enumerate() {
        print_job(i + 1, job);
    }

    Ok(())
}

fn print_job(index: usize, job: &Job) {
    println!("Job #{}: id={}", index, job.job_id());
    if let Some(printer) = job.printer_name() {
        println!("   Printer: {}", printer);
    }
    if let Some(name) = job.name() {
        println!("   Name: {}", name);
    }
    if let Some(document) = job.document() {
        println!("   Document: {}", document);
    }
    if let Some(owner) = job.owner() {
        println!("   Owner: {}", owner);
    }
    println!("   Status: {}", job.status().description());
    if let Some(status_string) = job.status_string() {
        println!("   Status string: {}", status_string);
    }
    if let Some(code) = job.job_status_code() {
        // Only Windows populates this - the raw `Win32_PrintJob.JobStatus`
        // bitmask. Linux backends leave it `None`.
        println!("   JobStatus bitmask: 0x{:08X}", code);
    }
    match (job.pages_printed(), job.total_pages()) {
        (Some(done), Some(total)) => println!("   Pages: {} / {}", done, total),
        (Some(done), None) => println!("   Pages printed: {}", done),
        (None, Some(total)) => println!("   Total pages: {}", total),
        (None, None) => {} // Backend didn't expose page counters.
    }
    if let Some(submitted) = job.time_submitted_raw() {
        println!("   Submitted (raw): {}", submitted);
    }
    println!();
}

fn print_platform_note() {
    #[cfg(unix)]
    println!(
        "Note (Linux): the default lpstat backend reports an empty list when \
         no jobs are spooled. Build with `--features linux-libcups` and rerun \
         for the libcups path."
    );
    #[cfg(windows)]
    println!(
        "Note (Windows): list_jobs queries Win32_PrintJob - it only returns \
         jobs currently spooled. Submit a print job and rerun to see entries."
    );
}
