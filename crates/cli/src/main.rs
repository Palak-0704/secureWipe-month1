
use securewipe_core::devices::detect_devices;
use securewipe_core::engine::wipe::perform_wipe;
use simplelog::*;
use std::fs::File;

fn main() {
    // Initialize logging to Month1-Submission/securewipe.log and to terminal
    let log_path = "../securewipe.log"; // relative to cli folder, puts log in Month1-Submission
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(log_path).unwrap()),
    ]).unwrap();

    log::info!("=== SecureWipe Device Detection Started ===");
    println!("\n=== SecureWipe Device Detection ===\n");
    let devices = detect_devices();
    if devices.is_empty() {
        println!("No storage devices detected. Please check your system or permissions.");
        log::warn!("No storage devices detected.");
        return;
    }
    for (i, device) in devices.iter().enumerate() {
        println!("Device [{}]", i + 1);
        println!("  Model:        {}", device.model);
        println!("  Type:         {}", device.dev_type);
        println!("  Size:         {} GB", device.size_gb);
        println!("  Serial:       {}", device.serial.as_deref().unwrap_or("N/A"));
        println!("  HPA/DCO:      {}", if device.hpa_dco { "Yes (hidden area detected)" } else { "No" });
        if !device.metadata.is_empty() {
            println!("  Metadata:");
            for (k, v) in &device.metadata {
                println!("    {}: {}", k, v);
            }
        }
        println!("----------------------------------------");
        log::info!("Device [{}]: model={}, type={}, size={}GB, serial={:?}, hpa_dco={}, metadata={:?}",
            i + 1, device.model, device.dev_type, device.size_gb, device.serial, device.hpa_dco, device.metadata);
    }
    // For integration: devices can be serialized to JSON for frontend use
    // serde_json::to_string_pretty(&devices) -> pass to GUI
    // Demo: perform wipe on first device
    if let Some(device) = devices.get(0) {
        log::info!("Performing wipe on device: {}", device.model);
        let result = perform_wipe(device);
        println!("\nWipe result for first device: {}\n", result);
        log::info!("Wipe result: {}", result);
    }
    log::info!("=== SecureWipe Device Detection Finished ===");
}
