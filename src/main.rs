
#[cfg(windows)]
use wmi::{COMLibrary, WMIConnection};
use serde::Deserialize;
use log::{info, error, warn};
use tokio::{self, time::{sleep, Duration}};
use std::env;

#[cfg(windows)]
#[derive(Deserialize, Debug)]
struct Win32_Printer {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "PrinterStatus")]
    printer_status: Option<u32>,
    #[serde(rename = "DetectedErrorState")]
    detected_error_state: Option<u32>,
    #[serde(rename = "WorkOffline")]
    work_offline: Option<bool>,
    #[serde(rename = "PrinterState")]
    printer_state: Option<u32>,
    #[serde(rename = "Default")]
    default: Option<bool>,
}

#[cfg(windows)]
impl Win32_Printer {
    fn get_status_description(&self) -> String {
        match self.printer_status {
            Some(1) => "Other".to_string(),
            Some(2) => "Unknown".to_string(),
            Some(3) => "Idle".to_string(),
            Some(4) => "Printing".to_string(),
            Some(5) => "Warmup".to_string(),
            Some(6) => "Stopped Printing".to_string(),
            Some(7) => "Offline".to_string(),
            _ => "Status Unknown".to_string(),
        }
    }

    fn get_error_description(&self) -> String {
        match self.detected_error_state {
            Some(0) => "No Error".to_string(),
            Some(1) => "Other".to_string(),
            Some(2) => "No Error".to_string(),
            Some(3) => "Low Paper".to_string(),
            Some(4) => "No Paper".to_string(),
            Some(5) => "Low Toner".to_string(),
            Some(6) => "No Toner".to_string(),
            Some(7) => "Door Open".to_string(),
            Some(8) => "Jammed".to_string(),
            Some(9) => "Service Requested".to_string(),
            Some(10) => "Output Bin Full".to_string(),
            _ => "Unknown Error State".to_string(),
        }
    }
}

#[cfg(windows)]
async fn check_printer_status() -> Result<Vec<Win32_Printer>, Box<dyn std::error::Error>> {
    info!("Initializing COM library...");
    let com_con = COMLibrary::new()?;
    
    info!("Establishing WMI connection...");
    let wmi_con = WMIConnection::new(com_con.into())?;
    
    info!("Querying printer information...");
    let printers: Vec<Win32_Printer> = wmi_con
        .async_query()
        .await?;
    
    Ok(printers)
}

#[cfg(windows)]
async fn find_printer_by_name(name: &str) -> Result<Option<Win32_Printer>, Box<dyn std::error::Error>> {
    let printers = check_printer_status().await?;
    
    for printer in printers {
        if let Some(printer_name) = &printer.name {
            if printer_name.eq_ignore_ascii_case(name) {
                return Ok(Some(printer));
            }
        }
    }
    
    Ok(None)
}

#[cfg(windows)]
async fn monitor_printer_service(printer_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting printer monitoring service for: {}", printer_name);
    println!("Monitoring printer '{}' every 60 seconds...", printer_name);
    println!("Press Ctrl+C to stop\n");
    
    let mut previous_status: Option<String> = None;
    let mut previous_error: Option<String> = None;
    
    loop {
        match find_printer_by_name(printer_name).await {
            Ok(Some(printer)) => {
                let current_status = printer.get_status_description();
                let current_error = printer.get_error_description();
                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                
                // Only log if status or error state changed
                println!("[{}] Checking printer '{}'", timestamp, printer_name);
                if previous_status.as_ref() != Some(&current_status) || 
                   previous_error.as_ref() != Some(&current_error) {
                    
                    println!("[{}] Printer '{}' Status Changed:", timestamp, printer_name);
                    println!("  Status: {}", current_status);
                    println!("  Error State: {}", current_error);
                    
                    if let Some(offline) = printer.work_offline {
                        println!("  Offline: {}", if offline { "Yes" } else { "No" });
                    }
                    
                    info!("Printer '{}' - Status: {}, Error: {}", printer_name, current_status, current_error);
                    
                    previous_status = Some(current_status);
                    previous_error = Some(current_error);
                    println!();
                } else {
                    // Just log to debug that we're still monitoring
                    info!("Printer '{}' status unchanged: {}", printer_name, current_status);
                }
            }
            Ok(None) => {
                warn!("Printer '{}' not found", printer_name);
                println!("[{}] Warning: Printer '{}' not found", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), printer_name);
            }
            Err(e) => {
                error!("Failed to check printer status: {}", e);
                println!("[{}] Error checking printer status: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), e);
            }
        }
        
        sleep(Duration::from_secs(10)).await;
    }
}

#[cfg(not(windows))]
async fn check_printer_status() -> Result<Vec<()>, Box<dyn std::error::Error>> {
    Err("Printer status checking is only supported on Windows".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        let printer_name = &args[1];
        
        #[cfg(windows)]
        {
            println!("Printer Status Monitor Service");
            println!("==============================");
            monitor_printer_service(printer_name).await?;
        }
        
        #[cfg(not(windows))]
        {
            println!("This application only supports Windows systems.");
            println!("Printer monitoring requires Windows Management Instrumentation (WMI).");
        }
    } else {
        println!("Printer Status Checker");
        println!("======================");
        println!("Usage:");
        println!("  {} <printer_name>    Monitor specific printer every 60 seconds", args[0]);
        println!("  {}                   List all printers once\n", args[0]);
        
        #[cfg(windows)]
        {
            match check_printer_status().await {
            Ok(printers) => {
                if printers.is_empty() {
                    println!("No printers found on this system.");
                } else {
                    println!("Found {} printer(s):\n", printers.len());
                    
                    for (i, printer) in printers.iter().enumerate() {
                        println!("Printer #{}: {}", 
                            i + 1, 
                            printer.name.as_ref().unwrap_or(&"Unknown Name".to_string())
                        );
                        
                        println!("  Status: {}", printer.get_status_description());
                        println!("  Error State: {}", printer.get_error_description());
                        
                        if let Some(offline) = printer.work_offline {
                            println!("  Offline: {}", if offline { "Yes" } else { "No" });
                        }
                        
                        if let Some(is_default) = printer.default {
                            if is_default {
                                println!("  Default Printer: Yes");
                            }
                        }
                        
                        println!();
                    }
                }
            }
            Err(e) => {
                error!("Failed to check printer status: {}", e);
                println!("Error: Failed to check printer status - {}", e);
                return Err(e);
            }
            }
        }
        
        #[cfg(not(windows))]
        {
            println!("This application only supports Windows systems.");
            println!("Printer status checking requires Windows Management Instrumentation (WMI).");
        }
    }
    
    Ok(())
}
