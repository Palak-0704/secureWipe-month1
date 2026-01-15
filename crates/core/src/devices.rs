//! src/devices.rs
//!
//! Device detection stub for Month 1 submission.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub dev_type: String, // "HDD" | "SSD" | "NVMe" | "USB" | "PHONE" | "OTHER"
    pub model: String,
    pub serial: Option<String>,
    pub size_gb: u64,
    pub encrypted: bool,
    pub hpa_dco: bool,
    pub firmware: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Real device detection using sysinfo (cross-platform, basic info)

/// Cross-platform device detection: Windows (WMIC), Linux (lsblk/udevadm), fallback to sysinfo
pub fn detect_devices() -> Vec<Device> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let mut devices = Vec::new();
        // Use WMIC to get disk info
        let output = Command::new("wmic")
            .args(["diskdrive", "get", "DeviceID,Model,SerialNumber,Size,MediaType"])
            .output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let id = parts[0].to_string();
                    let model = parts[1].to_string();
                    let serial = Some(parts[2].to_string());
                    let size_gb = parts[3].parse::<u64>().unwrap_or(0) / (1024 * 1024 * 1024);
                    let dev_type = parts[4].to_uppercase();
                    let dev_type = if dev_type.contains("SSD") {
                        "SSD"
                    } else if dev_type.contains("HDD") {
                        "HDD"
                    } else if dev_type.contains("USB") {
                        "USB"
                    } else {
                        "OTHER"
                    };
                    let mut metadata = HashMap::new();
                    metadata.insert("wmic_line".into(), line.to_string());
                    // HPA/DCO detection on Windows is vendor-specific and not available via WMIC.
                    // For advanced detection, integrate with vendor tools or use IOCTLs (future work).
                    let hpa_dco = false; // Stub: set to false for now
                    devices.push(Device {
                        id,
                        dev_type: dev_type.to_string(),
                        model,
                        serial,
                        size_gb,
                        encrypted: false,
                        hpa_dco,
                        firmware: None,
                        metadata,
                    });
                }
            }
        }
        devices
    }
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let mut devices = Vec::new();
        // Use lsblk to get disk info in JSON
        let output = Command::new("lsblk")
            .args(["-J", "-o", "NAME,MODEL,SERIAL,SIZE,TYPE,MOUNTPOINT"])
            .output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_default();
            if let Some(blockdevices) = json.get("blockdevices").and_then(|v| v.as_array()) {
                for dev in blockdevices {
                    let dev_type = dev.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if dev_type == "disk" {
                        let id = dev.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let model = dev.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let serial = dev.get("serial").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let size_str = dev.get("size").and_then(|v| v.as_str()).unwrap_or("0G");
                        let size_gb = if size_str.ends_with('G') {
                            size_str.trim_end_matches('G').parse::<u64>().unwrap_or(0)
                        } else { 0 };
                        let mut metadata = HashMap::new();
                        if let Some(mp) = dev.get("mountpoint").and_then(|v| v.as_str()) {
                            metadata.insert("mount_point".into(), mp.to_string());
                        }
                        // HPA/DCO detection using hdparm
                        let mut hpa_dco = false;
                        let hdparm_out = Command::new("hdparm")
                            .args(["-N", &format!("/dev/{}", id)])
                            .output();
                        if let Ok(hdparm_out) = hdparm_out {
                            let hdparm_str = String::from_utf8_lossy(&hdparm_out.stdout);
                            if hdparm_str.contains("HPA") || hdparm_str.contains("DCO") {
                                hpa_dco = true;
                                metadata.insert("hdparm".into(), hdparm_str.to_string());
                            }
                        }
                        devices.push(Device {
                            id,
                            dev_type: "DISK".to_string(),
                            model,
                            serial,
                            size_gb,
                            encrypted: false,
                            hpa_dco,
                            firmware: None,
                            metadata,
                        });
                    }
                }
            }
        }
        devices
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        // Fallback: sysinfo
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
            let size_gb = disk.total_space() / (1024 * 1024 * 1024);
            let mut metadata = HashMap::new();
            metadata.insert("mount_point".into(), disk.mount_point().to_string_lossy().to_string());
            devices.push(Device {
                id,
                dev_type,
                model,
                serial: None,
                size_gb,
                encrypted: false,
                hpa_dco: false,
                firmware: None,
                metadata,
            });
        }
        devices
    }
}
