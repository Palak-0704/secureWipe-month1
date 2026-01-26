use crate::platform;
// use std::path::Path;
use serde_json;

/// Unified API: Get current device state as JSON (for GUI/API)
pub fn get_device_state_json() -> String {
    match get_current_device_state() {
        Ok(devices) => serde_json::to_string_pretty(&devices).unwrap_or_else(|_| "[]".to_string()),
        Err(e) => format!("{{\"error\":\"{}\"}}", e),
    }
}

/// Thread-safe getter for device state (handles poisoned mutex)
#[cfg(target_os = "windows")]
pub fn get_current_device_state() -> Result<Vec<Device>, String> {
    // Use a let binding to extend the lifetime of the lock guard
    let guard = platform::DEVICE_STATE.lock().map_err(|e| {
        log::error!("Device state mutex poisoned: {}", e);
        "Device state unavailable".to_string()
    })?;
    let devices = guard.clone();
    println!("[DEBUG] Detected devices: {:#?}", devices);
    Ok(devices)
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::sync::{Arc, Mutex};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::thread;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
lazy_static::lazy_static! {
    static ref DEVICE_STATE: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
}

/// Start a background thread to monitor storage devices and partitions in real time (Linux/Mac).
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn start_device_monitoring(poll_interval_secs: u64) {
    let state = DEVICE_STATE.clone();
    thread::spawn(move || {
        loop {
            let devices = detect_devices_with_partitions_and_free_space();
            let mut state_guard = state.lock().unwrap();
            *state_guard = devices;
            drop(state_guard);
            thread::sleep(Duration::from_secs(poll_interval_secs));
        }
    });
}

/// Get the current device/partition state (thread-safe, Linux/Mac)
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn get_current_device_state() -> Vec<Device> {
    DEVICE_STATE.lock().unwrap().clone()
}

/// Enhanced device detection: includes partitions, free space, and encryption (Linux/Mac)
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn detect_devices_with_partitions_and_free_space() -> Vec<Device> {
    use std::collections::HashMap;
    use sysinfo::{System, SystemExt, DiskExt};
    let mut sys = System::new_all();
    sys.refresh_disks_list();
    let mut devices = Vec::new();
    for disk in sys.disks() {
        let id = format!("{}", disk.name().to_string_lossy());
        let dev_type = match disk.type_() {
            sysinfo::DiskType::HDD => "HDD",
            sysinfo::DiskType::SSD => "SSD",
            _ => "OTHER",
        }.to_string();
        let model = disk.name().to_string_lossy().to_string();
        let mut size_gb = disk.total_space() / (1024 * 1024 * 1024);
        let allocated_gb = Some(disk.total_space().saturating_sub(disk.available_space()) / (1024 * 1024 * 1024));
        let mut metadata = HashMap::new();
        metadata.insert("mount_point".into(), disk.mount_point().to_string_lossy().to_string());
        // Health, temperature, firmware (if available)
        let (smart_status, temperature_c, firmware) = get_advanced_metadata(&id);
        // Encryption detection (Linux: LUKS, Mac: FileVault)
        let encrypted = {
            #[cfg(target_os = "linux")]
            {
                let cryptsetup = std::process::Command::new("cryptsetup").arg("isLuks").arg(&id).output();
                if let Ok(out) = cryptsetup {
                    out.status.success()
                } else {
                    false
                }
            }
            #[cfg(target_os = "macos")]
            {
                let diskutil = std::process::Command::new("diskutil").arg("info").arg(&id).output();
                if let Ok(out) = diskutil {
                    let out_str = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    out_str.contains("encrypted: yes")
                } else {
                    false
                }
            }
        };
        let partitions = vec![{
            let name = disk.name().to_string_lossy().to_string();
            let mount_point = Some(disk.mount_point().to_string_lossy().to_string());
            let size_gb = size_gb;
            let used_gb = allocated_gb;
            let fs_type = Some(disk.file_system().to_string_lossy().to_string());
            Partition {
                name,
                mount_point,
                size_gb,
                used_gb,
                fs_type,
                is_system: None,
                is_boot: None,
                encrypted: Some(encrypted),
            }
        }];
        devices.push(Device {
            id,
            dev_type,
            model,
            serial: None,
            size_gb,
            allocated_gb,
            partitions,
            connection: None,
            removable: None,
            is_system: None,
            smart_status,
            temperature_c,
            encrypted,
            hpa_dco: false,
            firmware,
            error: None,
            metadata,
        });
    }
    devices
}
// use std::sync::{Arc, Mutex};
// use std::thread;
// use std::time::Duration;

// Windows-specific device state and detection are now in platform.rs

/// Fast storage usage calculation using statvfs (Linux/macOS) or GetDiskFreeSpaceEx (Windows)
pub fn calculate_total_storage_usage(root: &str) -> u64 {
    #[cfg(target_os = "windows")]
    {
        use std::mem::MaybeUninit;
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        use winapi::um::fileapi::GetDiskFreeSpaceExW;
        use winapi::shared::ntdef::ULARGE_INTEGER;
        let path: Vec<u16> = OsStr::new(root).encode_wide().chain(Some(0)).collect();
        unsafe {
            let mut free_bytes = MaybeUninit::<ULARGE_INTEGER>::uninit();
            let mut total_bytes = MaybeUninit::<ULARGE_INTEGER>::uninit();
            let mut avail_bytes = MaybeUninit::<ULARGE_INTEGER>::uninit();
            if GetDiskFreeSpaceExW(path.as_ptr(), free_bytes.as_mut_ptr(), total_bytes.as_mut_ptr(), avail_bytes.as_mut_ptr()) != 0 {
                let total_bytes = total_bytes.assume_init();
                let free_bytes = free_bytes.assume_init();
                let total = total_bytes.QuadPart();
                let free = free_bytes.QuadPart();
                return (total - free) as u64;
            }
        }
        0
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use nix::sys::statvfs::statvfs;
        if let Ok(stats) = statvfs(root) {
            let total = stats.blocks() * stats.block_size();
            let free = stats.blocks_free() * stats.block_size();
            return total - free;
        }
        0
    }
}
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Partition {
    pub name: String,
    pub mount_point: Option<String>,
    pub size_gb: u64,
    pub used_gb: Option<u64>,
    pub fs_type: Option<String>,
    pub is_system: Option<bool>,
    pub is_boot: Option<bool>,
    pub encrypted: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub id: String,
    pub dev_type: String,
    pub model: String,
    pub serial: Option<String>,
    pub size_gb: u64,
    pub allocated_gb: Option<u64>,
    pub partitions: Vec<Partition>,
    pub connection: Option<String>,
    pub removable: Option<bool>,
    pub is_system: Option<bool>,
    pub smart_status: Option<String>,
    pub temperature_c: Option<f32>,
    pub encrypted: bool,
    pub hpa_dco: bool,
    pub firmware: Option<String>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}
use std::fs;
use serde_json::Value as JsonValue;

/// Simple localization utility for error messages
fn get_localized_message(lang: &str, key: &str, args: &[(&str, &str)]) -> String {
    let locale_path = match lang {
        "hi" => "../../locales/hi/ui.json",
        _ => "../../locales/en/ui.json",
    };
    let json = fs::read_to_string(locale_path).ok()
        .and_then(|data| serde_json::from_str::<JsonValue>(&data).ok());
    let mut msg = json.and_then(|j| j.get(key).and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| key.to_string());
    for (k, v) in args {
        msg = msg.replace(&format!("{{{}}}", k), v);
    }
    msg
}
use log::error;
// --- Helper functions for advanced metadata and error handling ---
/// Stub: Get advanced metadata (SMART, temperature, firmware) for a device by id
pub fn get_advanced_metadata(_id: &str) -> (Option<String>, Option<f32>, Option<String>) {
    // TODO: Implement real SMART/temperature/firmware detection
    (None, None, None)
}

/// Stub: Handle device error and return a localized error message if needed
/// Error handler with localization support
pub fn handle_device_error(id: &str, model: &str, size_gb: u64) -> Option<String> {
    let lang = std::env::var("APP_LANG").unwrap_or_else(|_| "en".to_string());
    let _ = model; // silence unused variable warning
    if size_gb == 0 {
        return Some(get_localized_message(&lang, "device_error_size_zero", &[ ("id", id) ]));
    }
    // Example: add more error types as needed
    None
}
// Real device detection using sysinfo (cross-platform, basic info)
// Returns a vector of [Device]s detected on the system.
// Cross-platform device detection: Windows (WMIC), Linux (lsblk/udevadm), fallback to sysinfo
pub fn detect_devices() -> Vec<Device> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::collections::HashMap;
        let mut devices = Vec::new();
        let output = Command::new("wmic")
            .args(["diskdrive", "get", "DeviceID,Model,SerialNumber,Size,MediaType,Index"])
            .output();
        if let Err(e) = &output {
            error!("Failed to run wmic: {}", e);
            return devices;
        }
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines = stdout.lines().skip(1); // skip header
            for line in lines {
                let line = line.trim();
                if line.is_empty() { continue; }
                println!("[WMIC] line: '{}'", line);
                // Updated regex to match: '\\.\PHYSICALDRIVE0  0      Fixed hard disk media  NVMe Micron_2400_MTFDKBA512QFM  0000_0000_0000_0001_00A0_7523_42DE_01A6.  512105932800'
                // Split line by two or more spaces to extract columns
                // let parts: Vec<&str> = line.split(|c| c == ' ').filter(|s| !s.is_empty()).collect();
                // If split by single spaces, fallback to splitting by two or more spaces
                let columns: Vec<&str> = regex::Regex::new(r"\s{2,}").unwrap().split(line).filter(|s| !s.is_empty()).collect();
                if columns.len() == 6 {
                    println!("[WMIC] columns matched!");
                    let id = columns[0].to_string();
                    let _index = columns[1];
                    let media_type = columns[2].trim().to_string();
                    let model = columns[3].trim().to_string();
                    let serial = Some(columns[4].trim().to_string());
                    let size_bytes = columns[5].trim().parse::<u64>().unwrap_or(0);
                    let size_gb = size_bytes / (1024 * 1024 * 1024);

                    // Guess dev_type and connection from model/media_type
                    let (dev_type, connection) = if model.to_lowercase().contains("nvme") {
                        ("NVMe".to_string(), Some("NVMe".to_string()))
                    } else if model.to_lowercase().contains("ssd") {
                        ("SSD".to_string(), Some("SATA".to_string()))
                    } else if model.to_lowercase().contains("usb") || media_type.to_lowercase().contains("usb") {
                        ("USB".to_string(), Some("USB".to_string()))
                    } else if model.to_lowercase().contains("hdd") || media_type.to_lowercase().contains("fixed") {
                        ("HDD".to_string(), Some("SATA".to_string()))
                    } else {
                        ("Other".to_string(), None)
                    };

                    let mut metadata = HashMap::new();
                    metadata.insert("media_type".to_string(), media_type.clone());
                    if let Some(ref s) = serial { metadata.insert("serial_number".to_string(), s.clone()); }

                    devices.push(Device {
                        id,
                        dev_type,
                        model,
                        serial,
                        size_gb,
                        allocated_gb: None,
                        partitions: vec![],
                        connection,
                        removable: Some(media_type.to_lowercase().contains("removable") || media_type.to_lowercase().contains("usb")),
                        is_system: Some(true),
                        smart_status: None,
                        temperature_c: None,
                        encrypted: false,
                        hpa_dco: false,
                        firmware: None,
                        error: None,
                        metadata,
                    });
                } else {
                    println!("[WMIC] columns did NOT match this line");
                }
            }
        }
        devices
    }
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        use std::collections::HashMap;
        use serde_json::Value;
        let mut devices = Vec::new();
        let output = Command::new("lsblk")
            .args(["-b", "-J", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE,RM,RO,UUID,PARTLABEL,PARTUUID,BOOT,MODEL"])
            .output();
        if let Err(e) = &output {
            error!("Failed to run lsblk: {}", e);
            return devices;
        }
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let json = serde_json::from_str::<Value>(&stdout);
            if let Err(e) = &json {
                error!("Failed to parse lsblk JSON: {}", e);
            }
            if let Ok(json) = json {
                if let Some(blockdevices) = json.get("blockdevices").and_then(|v| v.as_array()) {
                    use std::thread;
                    let mut handles = Vec::new();
                    for line in lines {
                        let line = line.trim();
                        if line.is_empty() { continue; }
                        println!("[WMIC] line: '{}'", line);
                        let re = regex::Regex::new(r"^(\\.\\PHYSICALDRIVE\d+)\s+(\d+)\s+([\w\s]+?)\s{2,}(.+?)\s{2,}([\w_.-]+)\s{2,}(\d+)").unwrap();
                        if let Some(caps) = re.captures(line) {
                            println!("[WMIC] regex matched!");
                            let id = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                            let _index = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                            let media_type = caps.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
                            let model = caps.get(4).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
                            let serial = caps.get(5).map(|m| m.as_str().trim().to_string());
                            let size_bytes = caps.get(6).map(|m| m.as_str().parse::<u64>().unwrap_or(0)).unwrap_or(0);
                            let size_gb = size_bytes / (1024 * 1024 * 1024);
                            // ...existing device push logic...
                            devices.push(Device {
                                id,
                                dev_type,
                                model,
                                serial,
                                size_gb,
                                allocated_gb: None,
                                partitions: vec![],
                                connection,
                                removable: Some(media_type.to_lowercase().contains("removable") || media_type.to_lowercase().contains("usb")),
                                is_system: Some(true),
                                smart_status: None,
                                temperature_c: None,
                                encrypted: false,
                                hpa_dco: false,
                                firmware: None,
                                error: None,
                                metadata,
                            });
                        } else {
                            println!("[WMIC] regex did NOT match this line");
                        }
                            // --- Encryption detection (Linux, robust) ---
                            let encrypted = {
                                // Check for LUKS, dm-crypt, or crypttab
                                let cryptsetup = Command::new("cryptsetup").arg("isLuks").arg(&id).output();
                                if let Ok(out) = cryptsetup {
                                    out.status.success()
                                } else {
                                    // Fallback: check /etc/crypttab or lsblk -o type
                                    let lsblk = Command::new("lsblk").args(["-o", "type", &id]).output();
                                    if let Ok(lsblk) = lsblk {
                                        let out_str = String::from_utf8_lossy(&lsblk.stdout).to_lowercase();
                                        out_str.contains("crypt")
                                    } else {
                                        false
                                    }
                                }
                            };
                            let mut partitions = Vec::new();
                            if let Some(children) = dev.get("children").and_then(|v| v.as_array()) {
                                for part in children {
                                    let name = part.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let mount_point = part.get("mountpoint").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let size_gb = part.get("size").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok()).map(|b| b / (1024 * 1024 * 1024)).unwrap_or(0);
                                    let fs_type = part.get("fstype").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let is_boot = part.get("boot").and_then(|v| v.as_str()).map(|s| s == "*" || s == "1");
                                    // --- Partition encryption detection (Linux, robust) ---
                                    let part_encrypted = {
                                        let cryptsetup = Command::new("cryptsetup").arg("isLuks").arg(&name).output();
                                        if let Ok(out) = cryptsetup {
                                            out.status.success()
                                        } else {
                                            // Fallback: check lsblk -o type
                                            let lsblk = Command::new("lsblk").args(["-o", "type", &name]).output();
                                            if let Ok(lsblk) = lsblk {
                                                let out_str = String::from_utf8_lossy(&lsblk.stdout).to_lowercase();
                                                out_str.contains("crypt")
                                            } else {
                                                false
                                            }
                                        }
                                    };
                                    // --- Filesystem and usage stats ---
                                    let (used_gb, fs_type) = if let Some(ref mount) = mount_point {
                                        let df_out = Command::new("df").arg("-B1").arg(mount).output();
                                        if let Ok(df_out) = df_out {
                                            let df_str = String::from_utf8_lossy(&df_out.stdout);
                                            let mut used = None;
                                            let mut fstype = fs_type.clone();
                                            for dline in df_str.lines().skip(1) {
                                                let dparts: Vec<&str> = dline.split_whitespace().collect();
                                                if dparts.len() >= 6 {
                                                    if let Ok(used_bytes) = dparts[2].parse::<u64>() {
                                                        used = Some(used_bytes / (1024 * 1024 * 1024));
                                                    }
                                                    if fstype.is_none() {
                                                        fstype = Some(dparts[0].to_string());
                                                    }
                                                    break;
                                                }
                                            }
                                            (used, fstype)
                                        } else {
                                            (None, fs_type.clone())
                                        }
                                    } else { (None, fs_type.clone()) };
                                    partitions.push(Partition {
                                        name: name.to_string(),
                                        mount_point,
                                        size_gb,
                                        used_gb,
                                        fs_type,
                                        is_system: None,
                                        is_boot,
                                        encrypted: Some(part_encrypted),
                                    });
                                }
                            }
                            let (smart_status, temperature_c, firmware) = get_advanced_metadata(&id);
                            let mut error = handle_device_error(&id, &model, size_gb);
                            if size_gb == 0 {
                                let msg = handle_device_error(&id, &model, size_gb).unwrap_or_else(|| format!("Device {} has size 0 or failed to parse size.", id));
                                warn!("{}", msg);
                                error = Some(msg);
                            }
                            devices.push(Device {
                                id: id.to_string(),
                                dev_type: "disk".to_string(),
                                model: model.to_string(),
                                serial: None,
                                size_gb,
                                allocated_gb: None,
                                partitions,
                                connection: None,
                                removable: dev.get("rm").and_then(|v| v.as_u64()).map(|v| v == 1),
                                is_system: None,
                                smart_status,
                                temperature_c,
                                encrypted,
                                hpa_dco,
                                firmware,
                                error,
                                metadata,
                            });
                        }
                    }
                }
            }
        }
        // End of Linux block
    }
    // Non-Windows, non-Linux fallback handled by top-level function
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn detect_devices_sysinfo() -> Vec<Device> {
        use sysinfo::{System, SystemExt, DiskExt};
        use std::collections::HashMap;
        let mut sys = System::new_all();
        sys.refresh_disks_list();
        let mut devices = Vec::new();
        for disk in sys.disks() {
            let id = format!("{}", disk.name().to_string_lossy());
            let dev_type = match disk.type_() {
                sysinfo::DiskType::HDD => "HDD",
                sysinfo::DiskType::SSD => "SSD",
                _ => "OTHER",
            }.to_string();
            let model = disk.name().to_string_lossy().to_string();
            let mut size_gb = disk.total_space() / (1024 * 1024 * 1024);
            if size_gb == 0 {
                if let Some(gb_match) = model.split_whitespace().find(|s| s.to_uppercase().ends_with("GB")) {
                    let num = gb_match.trim_end_matches(|c: char| !c.is_digit(10)).parse::<u64>().unwrap_or(0);
                    if num > 0 { size_gb = num; }
                } else {
                    warn!("Device {} has size 0 and no GB marker in model string.", id);
                }
            }
            let allocated_gb = Some(disk.total_space().saturating_sub(disk.available_space()) / (1024 * 1024 * 1024));
            let mut metadata = HashMap::new();
            metadata.insert("mount_point".into(), disk.mount_point().to_string_lossy().to_string());
            let partitions = vec![{
                let name = disk.name().to_string_lossy().to_string();
                let mount_point = Some(disk.mount_point().to_string_lossy().to_string());
                let size_gb = size_gb;
                let used_gb = allocated_gb;
                let fs_type = Some(disk.file_system().to_string_lossy().to_string());
                Partition {
                    name,
                    mount_point,
                    size_gb,
                    used_gb,
                    fs_type,
                    is_system: None,
                    is_boot: None,
                    encrypted: None,
                }
            }];
            let (connection, removable, smart_status, _error) = get_macos_sysinfo_enhancements(&id, &model);
            let firmware = None;
            // --- HPA/DCO detection (Mac): Not available, fallback to false ---
            let hpa_dco = false;
            // --- Encryption detection (Mac, robust) ---
            let encrypted = {
                let diskutil = std::process::Command::new("diskutil").arg("info").arg(&id).output();
                if let Ok(out) = diskutil {
                    let out_str = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    if out_str.contains("encrypted: yes") {
                        true
                    } else if let Some(line) = out_str.lines().find(|l| l.contains("encryption type")) {
                        !line.contains("none")
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            let mut error = handle_device_error(&id, &model, size_gb);
            if size_gb == 0 {
                let msg = handle_device_error(&id, &model, size_gb).unwrap_or_else(|| format!("Device {} has size 0 or failed to parse size.", id));
                warn!("{}", msg);
                error = Some(msg);
            }
            devices.push(Device {
                id,
                dev_type,
                model,
                serial: None,
                size_gb,
                allocated_gb,
                partitions,
                connection,
                removable,
                is_system: None,
                smart_status,
                temperature_c: None,
                encrypted,
                hpa_dco,
                firmware,
                error,
                metadata,
            });
        }
        devices
    }
    // End of detect_devices

