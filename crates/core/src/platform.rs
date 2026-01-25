//! Platform-specific device/partition detection and monitoring.
//!
//! Use `start_device_event_monitoring()` for event-driven monitoring (where supported),
//! or `start_device_monitoring()` for polling-based fallback.

pub use crate::devices::{Partition, Device};
// Platform-specific imports are inside each module

// Re-export DEVICE_STATE for all platforms
#[cfg(target_os = "windows")]
pub use imp::DEVICE_STATE;
#[cfg(target_os = "linux")]
pub use imp::DEVICE_STATE;
#[cfg(target_os = "macos")]
pub use imp::DEVICE_STATE;

#[cfg(target_os = "windows")]
pub mod imp {
    /// Start device monitoring using Windows device notification API if available, otherwise fallback to polling.
    pub fn start_device_event_monitoring(poll_interval_secs: u64) {
        warn!("Event-driven device monitoring for Windows is not implemented in CLI; falling back to polling.");
        start_device_monitoring(poll_interval_secs);
    }
    use crate::platform::{Device, Partition};
    use std::collections::HashMap;
    use std::process::Command;
    use lazy_static::lazy_static;
    use log::warn;
    lazy_static! {
        pub static ref DEVICE_STATE: std::sync::Arc<std::sync::Mutex<Vec<Device>>> = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    }
    pub fn start_device_monitoring(poll_interval_secs: u64) {
        let state = DEVICE_STATE.clone();
        std::thread::spawn(move || {
            loop {
                if let Ok(devices) = detect_devices_with_partitions_and_free_space() {
                    if let Ok(mut state_guard) = state.lock() {
                        *state_guard = devices;
                    } else {
                        warn!("Device state mutex poisoned (Windows)");
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(poll_interval_secs));
            }
        });
    }

    pub fn detect_devices_with_partitions_and_free_space() -> Result<Vec<Device>, String> {
        let mut devices = Vec::new();
        let output = Command::new("wmic")
            .args(["diskdrive", "get", "DeviceID,Model,SerialNumber,Size,MediaType,Index"])
            .output()
            .map_err(|e| format!("Failed to run wmic: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() { continue; }
            let columns: Vec<&str> = regex::Regex::new(r"\s{2,}").unwrap().split(line).filter(|s| !s.is_empty()).collect();
            if columns.len() != 6 {
                warn!("WMIC output parse error: '{}', columns: {:?}", line, columns);
                continue;
            }
            let id = columns[0].to_string();
            let index = columns[1].to_string();
            let media_type = columns[2].trim().to_string();
            let model = columns[3].trim().to_string();
            let serial = Some(columns[4].trim().to_string());
            let size_bytes = columns[5].trim().parse::<u64>().unwrap_or(0);
            let size_gb = size_bytes / (1024 * 1024 * 1024);
            // Get partitions for this drive
            let mut partitions = Vec::new();
            let part_output = Command::new("wmic")
                .args(["partition", "where", &format!("DiskIndex={}", index), "get", "DeviceID,Name,Size,Type,Index"])
                .output();
            if let Ok(part_output) = part_output {
                let part_stdout = String::from_utf8_lossy(&part_output.stdout);
                for pline in part_stdout.lines().skip(1) {
                    let pline = pline.trim();
                    if pline.is_empty() { continue; }
                    let pcols: Vec<&str> = regex::Regex::new(r"\s{2,}").unwrap().split(pline).filter(|s| !s.is_empty()).collect();
                    if pcols.len() < 3 { continue; }
                    let pname = pcols[1].to_string();
                    let psize_gb = pcols[2].trim().parse::<u64>().unwrap_or(0) / (1024 * 1024 * 1024);
                    // Get free space for this partition (using logicaldisk)
                    let mut mount_point = None;
                    let mut used_gb = None;
                    let mut fs_type = None;
                    let mut encrypted = None;
                    let ld_output = Command::new("wmic")
                        .args(["logicaldisk", "where", &format!("DeviceID='{}'", pname), "get", "DeviceID,FileSystem,FreeSpace,Size,VolumeName"])
                        .output();
                    if let Ok(ld_output) = ld_output {
                        let ld_stdout = String::from_utf8_lossy(&ld_output.stdout);
                        for ldline in ld_stdout.lines().skip(1) {
                            let ldline = ldline.trim();
                            if ldline.is_empty() { continue; }
                            let ldcols: Vec<&str> = regex::Regex::new(r"\s{2,}").unwrap().split(ldline).filter(|s| !s.is_empty()).collect();
                            if ldcols.len() >= 4 {
                                mount_point = Some(ldcols[0].to_string());
                                fs_type = Some(ldcols[1].to_string());
                                let free_bytes = ldcols[2].parse::<u64>().unwrap_or(0);
                                let total_bytes = ldcols[3].parse::<u64>().unwrap_or(0);
                                used_gb = Some((total_bytes.saturating_sub(free_bytes)) / (1024 * 1024 * 1024));
                            }
                        }
                    }
                    // Check BitLocker status (encrypted)
                    let bl_output = Command::new("manage-bde")
                        .args(["-status", &pname])
                        .output();
                    if let Ok(bl_output) = bl_output {
                        let bl_stdout = String::from_utf8_lossy(&bl_output.stdout);
                        if bl_stdout.contains("Percentage Encrypted: 100%") || bl_stdout.contains("Protection Status: Protection On") {
                            encrypted = Some(true);
                        } else if bl_stdout.contains("Protection Status: Protection Off") {
                            encrypted = Some(false);
                        }
                    }
                    partitions.push(Partition {
                        name: pname,
                        mount_point,
                        size_gb: psize_gb,
                        used_gb,
                        fs_type,
                        is_system: None,
                        is_boot: None,
                        encrypted,
                    });
                }
            }
            let mut metadata = HashMap::new();
            metadata.insert("media_type".to_string(), media_type.clone());
            if let Some(ref s) = serial { metadata.insert("serial_number".to_string(), s.clone()); }
            let (smart_status, temperature_c, firmware) = get_advanced_metadata_windows(&id);
            devices.push(Device {
                id,
                dev_type: if model.to_lowercase().contains("nvme") { "NVMe".to_string() } else if model.to_lowercase().contains("ssd") { "SSD".to_string() } else if model.to_lowercase().contains("usb") || media_type.to_lowercase().contains("usb") { "USB".to_string() } else if model.to_lowercase().contains("hdd") || media_type.to_lowercase().contains("fixed") { "HDD".to_string() } else { "Other".to_string() },
                model,
                serial,
                size_gb,
                allocated_gb: None,
                partitions,
                connection: None,
                removable: Some(media_type.to_lowercase().contains("removable") || media_type.to_lowercase().contains("usb")),
                is_system: Some(true),
                smart_status,
                temperature_c,
                encrypted: false,
                hpa_dco: false,
                firmware,
                error: None,
                metadata,
            });
        }
        Ok(devices)
    }

    /// Collect SMART, firmware, and temperature info for a device on Windows using WMIC and PowerShell.
    fn get_advanced_metadata_windows(dev_id: &str) -> (Option<String>, Option<f32>, Option<String>) {
        use std::process::Command;
        // SMART status via WMIC
        let smart_status = Command::new("wmic")
            .args(["diskdrive", "where", &format!("DeviceID='{}'", dev_id), "get", "Status"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                s.lines().skip(1).next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
            });
        // Firmware revision via WMIC
        let firmware = Command::new("wmic")
            .args(["diskdrive", "where", &format!("DeviceID='{}'", dev_id), "get", "FirmwareRevision"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                s.lines().skip(1).next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
            });
        // Temperature via PowerShell (best effort, not all drives support)
        let temperature_c = Command::new("powershell")
            .args(["-Command", &format!(r#"Get-WmiObject -Namespace root\wmi -Class MSStorageDriver_ATAPISmartData | ForEach-Object {{ if ($_.InstanceName -like '*{}*') {{ $t = $_.VendorSpecific[115]; if ($t -is [int]) {{ $t }} }} }}"#, dev_id.replace("\\", "").replace(".", ""))])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                s.trim().parse::<f32>().ok()
            });
        (smart_status, temperature_c, firmware)
    }
}

#[cfg(target_os = "linux")]
pub mod imp {
    use crate::platform::{Device, Partition};
    use std::collections::HashMap;
    use std::process::Command;
    use lazy_static::lazy_static;
    use log::warn;
    use std::sync::{Arc, Mutex};
    lazy_static! {
        pub static ref DEVICE_STATE: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
    }
    /// Start device monitoring using udev events if available, otherwise fallback to polling.
    pub fn start_device_event_monitoring(poll_interval_secs: u64) {
        let state = DEVICE_STATE.clone();
        std::thread::spawn(move || {
            if let Ok(mut monitor) = udev::MonitorBuilder::new().and_then(|b| b.match_subsystem_devtype("block", None)).and_then(|b| b.listen()) {
                for event in monitor {
                    match detect_devices_with_partitions_and_free_space() {
                        Ok(devices) => {
                            if let Ok(mut state_guard) = state.lock() {
                                *state_guard = devices;
                            } else {
                                warn!("Device state mutex poisoned (Linux, udev)");
                            }
                        }
                        Err(e) => warn!("Device detection error: {}", e),
                    }
                }
            } else {
                loop {
                    match detect_devices_with_partitions_and_free_space() {
                        Ok(devices) => {
                            if let Ok(mut state_guard) = state.lock() {
                                *state_guard = devices;
                            } else {
                                warn!("Device state mutex poisoned (Linux, poll)");
                            }
                        }
                        Err(e) => warn!("Device detection error: {}", e),
                    }
                    std::thread::sleep(std::time::Duration::from_secs(poll_interval_secs));
                }
            }
        });
    }
    /// Legacy polling-based monitoring (for compatibility)
    pub fn start_device_monitoring(poll_interval_secs: u64) {
        let state = DEVICE_STATE.clone();
        std::thread::spawn(move || {
            loop {
                match detect_devices_with_partitions_and_free_space() {
                    Ok(devices) => {
                        if let Ok(mut state_guard) = state.lock() {
                            *state_guard = devices;
                        } else {
                            warn!("Device state mutex poisoned (Linux)");
                        }
                    }
                    Err(e) => warn!("Device detection error: {}", e),
                }
                std::thread::sleep(std::time::Duration::from_secs(poll_interval_secs));
            }
        });
    }
    pub fn detect_devices_with_partitions_and_free_space() -> Result<Vec<Device>, String> {
        let mut devices = Vec::new();
        // Use lsblk to get all block devices and partitions in JSON
        let output = Command::new("lsblk")
            .args(["-J", "-o", "NAME,KNAME,TYPE,SIZE,MODEL,SERIAL,FSTYPE,MOUNTPOINT,UUID,ROTA,RM,STATE,TRAN"])
            .output()
            .map_err(|e| format!("Failed to run lsblk: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| format!("lsblk JSON parse error: {}", e))?;
        let blockdevices = json["blockdevices"].as_array().ok_or("lsblk output missing blockdevices")?;
        for dev in blockdevices {
            if dev["type"] == "disk" {
                let id = match dev["name"].as_str() {
                    Some(name) => format!("/dev/{}", name),
                    None => {
                        warn!("lsblk: missing device name");
                        continue;
                    }
                };
                let model = dev["model"].as_str().unwrap_or("").to_string();
                let serial = dev["serial"].as_str().map(|s| s.to_string());
                let size_gb = dev["size"].as_str().and_then(|s| s.parse::<f64>().ok()).map(|s| (s * 1.0) as u64).unwrap_or(0);
                let dev_type = if dev["rota"].as_u64() == Some(0) { "SSD" } else { "HDD" };
                let connection = dev["tran"].as_str().map(|s| s.to_string());
                let mut metadata = HashMap::new();
                if let Some(m) = dev["model"].as_str() { metadata.insert("model".to_string(), m.to_string()); }
                if let Some(s) = dev["serial"].as_str() { metadata.insert("serial_number".to_string(), s.to_string()); }
                if let Some(t) = dev["tran"].as_str() { metadata.insert("connection".to_string(), t.to_string()); }
                // Advanced metadata: SMART, firmware, temperature
                let (smart_status, temperature_c, firmware) = get_advanced_metadata_linux(&id);
                // Partitions
                let mut partitions = Vec::new();
                if let Some(children) = dev["children"].as_array() {
                    for part in children {
                        let pname = match part["name"].as_str() {
                            Some(name) => format!("/dev/{}", name),
                            None => {
                                warn!("lsblk: missing partition name");
                                continue;
                            }
                        };
                        let size_gb = part["size"].as_str().and_then(|s| s.parse::<f64>().ok()).map(|s| (s * 1.0) as u64).unwrap_or(0);
                        let mount_point = part["mountpoint"].as_str().map(|s| s.to_string());
                        let fs_type = part["fstype"].as_str().map(|s| s.to_string());
                        let encrypted = detect_luks_encryption(&pname);
                        let used_gb = mount_point.as_ref().and_then(|mp| get_used_gb(mp));
                        partitions.push(Partition {
                            name: pname,
                            mount_point,
                            size_gb,
                            used_gb,
                            fs_type,
                            is_system: None,
                            is_boot: None,
                            encrypted: Some(encrypted),
                        });
                    }
                }
                devices.push(Device {
                    id,
                    dev_type: dev_type.to_string(),
                    model,
                    serial,
                    size_gb,
                    allocated_gb: None,
                    partitions,
                    connection,
                    removable: dev["rm"].as_u64().map(|v| v != 0),
                    is_system: None,
                    smart_status,
                    temperature_c,
                    encrypted: false,
                    hpa_dco: false,
                    firmware,
                    error: None,
                    metadata,
                });
            }
        }
        Ok(devices)
    }
    fn get_advanced_metadata_linux(dev: &str) -> (Option<String>, Option<f32>, Option<String>) {
        // Use smartctl for SMART status, temperature, firmware
        let output = Command::new("smartctl").args(["-a", dev]).output();
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let smart_status = if stdout.contains("PASSED") {
                Some("PASSED".to_string())
            } else if stdout.contains("FAILED") {
                Some("FAILED".to_string())
            } else {
                None
            };
            let temperature_c = stdout.lines().find_map(|l| {
                if l.to_lowercase().contains("temperature") {
                    l.split_whitespace().filter_map(|w| w.parse::<f32>().ok()).next()
                } else { None }
            });
            let firmware = stdout.lines().find_map(|l| {
                if l.to_lowercase().contains("firmware") {
                    l.split(':').nth(1).map(|s| s.trim().to_string())
                } else { None }
            });
            (smart_status, temperature_c, firmware)
        } else {
            (None, None, None)
        }
    }
    fn detect_luks_encryption(dev: &str) -> bool {
        let output = Command::new("cryptsetup").args(["isLuks", dev]).output();
        matches!(output, Ok(ref o) if o.status.success())
    }
    fn get_used_gb(mount_point: &str) -> Option<u64> {
        let output = Command::new("df").args(["-B1", mount_point]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().nth(1).and_then(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            if cols.len() >= 3 {
                let used = cols[2].parse::<u64>().ok()?;
                Some(used / (1024 * 1024 * 1024))
            } else { None }
        })
    }
}

#[cfg(target_os = "macos")]
pub mod imp {
    use crate::platform::{Device, Partition};
    use std::collections::HashMap;
    use std::process::Command;
    use lazy_static::lazy_static;
    use log::{warn, error};
    use std::sync::{Arc, Mutex};
    lazy_static! {
        pub static ref DEVICE_STATE: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
    }
    pub fn start_device_monitoring(poll_interval_secs: u64) {
        let state = DEVICE_STATE.clone();
        std::thread::spawn(move || {
            loop {
                match detect_devices_with_partitions_and_free_space() {
                    Ok(devices) => {
                        if let Ok(mut state_guard) = state.lock() {
                            *state_guard = devices;
                        } else {
                            warn!("Device state mutex poisoned (macOS)");
                        }
                    }
                    Err(e) => warn!("Device detection error: {}", e),
                }
                std::thread::sleep(std::time::Duration::from_secs(poll_interval_secs));
            }
        });
    }
    pub fn detect_devices_with_partitions_and_free_space() -> Result<Vec<Device>, String> {
        let mut devices = Vec::new();
        // Use diskutil to get all disks and partitions in plist (XML) format
        let output = Command::new("diskutil").args(["list", "-plist"]).output().map_err(|e| format!("Failed to run diskutil: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let plist: plist::Value = plist::Value::from_reader_xml(stdout.as_bytes()).map_err(|e| format!("diskutil plist parse error: {}", e))?;
        let all_disks = plist.as_dictionary().and_then(|d| d.get("AllDisksAndPartitions")).and_then(|v| v.as_array()).ok_or("diskutil plist missing AllDisksAndPartitions")?;
        for dev in all_disks {
            let disk = dev.as_dictionary().ok_or("diskutil: disk entry not a dict")?;
            let id = disk.get("DeviceIdentifier").and_then(|v| v.as_string()).map(|s| format!("/dev/{}", s)).unwrap_or_default();
            let model = disk.get("Content").and_then(|v| v.as_string()).unwrap_or("").to_string();
            let size_gb = disk.get("Size").and_then(|v| v.as_unsigned_integer()).map(|s| s / (1024 * 1024 * 1024)).unwrap_or(0);
            let dev_type = if disk.get("Internal").and_then(|v| v.as_boolean()).unwrap_or(false) { "Internal" } else { "External" };
            let mut metadata = HashMap::new();
            if let Some(s) = disk.get("Content").and_then(|v| v.as_string()) { metadata.insert("content".to_string(), s.to_string()); }
            // Advanced metadata: SMART, firmware, temperature
            let (smart_status, temperature_c, firmware) = get_advanced_metadata_macos(&id);
            // Partitions
            let mut partitions = Vec::new();
            if let Some(parts) = disk.get("Partitions").and_then(|v| v.as_array()) {
                for part in parts {
                    let pd = part.as_dictionary().ok_or("diskutil: partition not a dict")?;
                    let pname = pd.get("DeviceIdentifier").and_then(|v| v.as_string()).map(|s| format!("/dev/{}", s)).unwrap_or_default();
                    let size_gb = pd.get("Size").and_then(|v| v.as_unsigned_integer()).map(|s| s / (1024 * 1024 * 1024)).unwrap_or(0);
                    let mount_point = pd.get("MountPoint").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let fs_type = pd.get("VolumeType").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let encrypted = detect_filevault_encryption(&pname);
                    let used_gb = mount_point.as_ref().and_then(|mp| get_used_gb(mp));
                    partitions.push(Partition {
                        name: pname,
                        mount_point,
                        size_gb,
                        used_gb,
                        fs_type,
                        is_system: None,
                        is_boot: None,
                        encrypted: Some(encrypted),
                    });
                }
            }
            devices.push(Device {
                id,
                dev_type: dev_type.to_string(),
                model,
                serial: None,
                size_gb,
                allocated_gb: None,
                partitions,
                connection: None,
                removable: None,
                is_system: None,
                smart_status,
                temperature_c,
                encrypted: false,
                hpa_dco: false,
                firmware,
                error: None,
                metadata,
            });
        }
        Ok(devices)
    }
    fn get_advanced_metadata_macos(dev: &str) -> (Option<String>, Option<f32>, Option<String>) {
        // Use smartmontools if available, or ioreg for temperature/firmware
        let output = Command::new("smartctl").args(["-a", dev]).output();
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let smart_status = if stdout.contains("PASSED") {
                Some("PASSED".to_string())
            } else if stdout.contains("FAILED") {
                Some("FAILED".to_string())
            } else {
                None
            };
            let temperature_c = stdout.lines().find_map(|l| {
                if l.to_lowercase().contains("temperature") {
                    l.split_whitespace().filter_map(|w| w.parse::<f32>().ok()).next()
                } else { None }
            });
            let firmware = stdout.lines().find_map(|l| {
                if l.to_lowercase().contains("firmware") {
                    l.split(':').nth(1).map(|s| s.trim().to_string())
                } else { None }
            });
            (smart_status, temperature_c, firmware)
        } else {
            (None, None, None)
        }
    }
    fn detect_filevault_encryption(dev: &str) -> bool {
        let output = Command::new("diskutil").args(["info", dev]).output();
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains("Encrypted: Yes")
        } else {
            false
        }
    }
    fn get_used_gb(mount_point: &str) -> Option<u64> {
        let output = Command::new("df").args(["-k", mount_point]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().nth(1).and_then(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            if cols.len() >= 3 {
                let used = cols[2].parse::<u64>().ok()?;
                Some(used / (1024 * 1024))
            } else { None }
        })
    }
}

// Re-export for unified API
#[cfg(target_os = "windows")]
pub use imp::*;
#[cfg(target_os = "linux")]
pub use imp::*;
#[cfg(target_os = "macos")]
pub use imp::*;
