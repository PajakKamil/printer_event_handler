
#[cfg(windows)]
use wmi::{COMLibrary, WMIConnection};
use serde::Deserialize;
use log::{info, error};
use tokio;

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

#[cfg(not(windows))]
async fn check_printer_status() -> Result<Vec<()>, Box<dyn std::error::Error>> {
    Err("Printer status checking is only supported on Windows".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("Printer Status Checker");
    println!("======================");
    
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
    Ok(())
}
