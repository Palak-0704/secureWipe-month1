use crate::devices::Device;

#[cfg(feature = "real_erase")]
use std::{
    fs,
    process::Command,
};

pub trait OfflineWipeExecutor {
    fn execute(&self, device: &Device) -> Result<String, String>;
}

#[cfg(feature = "real_erase")]
pub struct CommandOfflineWipeExecutor {
    executable: String,
    args_template: Vec<String>,
}

#[cfg(feature = "real_erase")]
pub struct NativeOfflineWipeExecutor;

#[cfg(feature = "real_erase")]
impl CommandOfflineWipeExecutor {
    pub fn from_env() -> Result<Self, String> {
        let executable = std::env::var("SECUREWIPE_ERASE_EXECUTABLE")
            .map_err(|_| {
                "Destructive wipe blocked: SECUREWIPE_ERASE_EXECUTABLE is required for custom offline erase executor configuration."
                    .to_string()
            })?
            .trim()
            .to_string();

        if executable.is_empty() {
            return Err(
                "Destructive wipe blocked: SECUREWIPE_ERASE_EXECUTABLE cannot be empty."
                    .to_string(),
            );
        }

        let args_template = std::env::var("SECUREWIPE_ERASE_ARGS_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                serde_json::from_str::<Vec<String>>(&value).map_err(|_| {
                    "Destructive wipe blocked: SECUREWIPE_ERASE_ARGS_JSON must be a JSON array of strings."
                        .to_string()
                })
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            executable,
            args_template,
        })
    }

    fn resolve_args(&self, device: &Device) -> Vec<String> {
        self.args_template
            .iter()
            .map(|arg| apply_placeholders(arg, device))
            .collect()
    }
}

#[cfg(feature = "real_erase")]
impl NativeOfflineWipeExecutor {
    fn build_windows_diskpart_script(device: &Device) -> Result<String, String> {
        let disk_number = parse_windows_disk_number(&device.id).ok_or_else(|| {
            format!(
                "Native erase failed: could not parse disk number from device id '{}'. Set SECUREWIPE_ERASE_EXECUTABLE to override.",
                device.id
            )
        })?;

        Ok(format!(
            "select disk {}\r\nattributes disk clear readonly\r\nonline disk noerr\r\nclean all\r\nexit\r\n",
            disk_number
        ))
    }

    #[cfg(target_os = "linux")]
    fn build_unix_device_path(device: &Device) -> String {
        if device.id.starts_with("/dev/") {
            device.id.clone()
        } else {
            format!("/dev/{}", device.id)
        }
    }
}

#[cfg(feature = "real_erase")]
impl OfflineWipeExecutor for CommandOfflineWipeExecutor {
    fn execute(&self, device: &Device) -> Result<String, String> {
        let args = self.resolve_args(device);
        let output = Command::new(&self.executable)
            .args(args)
            .output()
            .map_err(|e| {
                format!(
                    "Destructive wipe failed to start executor '{}': {}",
                    self.executable, e
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let details = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "executor returned non-zero status with no output".to_string()
            };
            return Err(format!(
                "Destructive wipe executor reported failure: {}",
                details
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if stdout.is_empty() {
            format!(
                "Offline destructive wipe executor completed for device: {} ({})",
                device.model, device.id
            )
        } else {
            stdout
        })
    }
}

#[cfg(feature = "real_erase")]
impl OfflineWipeExecutor for NativeOfflineWipeExecutor {
    fn execute(&self, device: &Device) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        {
            let script = Self::build_windows_diskpart_script(device)?;
            let script_path = std::env::temp_dir().join(format!(
                "securewipe-diskpart-{}.txt",
                sanitize_for_filename(&device.id)
            ));

            fs::write(&script_path, script.as_bytes()).map_err(|e| {
                format!(
                    "Native erase failed: unable to write diskpart script '{}': {}",
                    script_path.display(),
                    e
                )
            })?;

            let output = Command::new("diskpart")
                .arg("/s")
                .arg(&script_path)
                .output()
                .map_err(|e| format!("Native erase failed: unable to run diskpart: {}", e))?;

            let _ = fs::remove_file(&script_path);

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Err(format!(
                    "Native erase failed via diskpart. {}",
                    if !stderr.is_empty() {
                        stderr
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        "diskpart returned non-zero status with no output".to_string()
                    }
                ));
            }

            return Ok(format!(
                "Native destructive erase completed via diskpart for device {} ({})",
                device.model, device.id
            ));
        }

        #[cfg(target_os = "linux")]
        {
            let path = Self::build_unix_device_path(device);
            let output = Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "dd if=/dev/zero of='{}' bs=16M oflag=direct conv=fsync status=none",
                    path
                ))
                .output()
                .map_err(|e| format!("Native erase failed: unable to run dd: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Err(format!(
                    "Native erase failed via dd for '{}'. {}",
                    path,
                    if !stderr.is_empty() {
                        stderr
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        "dd returned non-zero status with no output".to_string()
                    }
                ));
            }

            return Ok(format!(
                "Native destructive erase completed via dd for device {} ({})",
                device.model, device.id
            ));
        }

        #[cfg(target_os = "macos")]
        {
            let target = if device.id.starts_with("/dev/") {
                device.id.replacen("/dev/", "", 1)
            } else {
                device.id.clone()
            };

            let output = Command::new("diskutil")
                .args(["secureErase", "0", &target])
                .output()
                .map_err(|e| format!("Native erase failed: unable to run diskutil: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Err(format!(
                    "Native erase failed via diskutil for '{}'. {}",
                    target,
                    if !stderr.is_empty() {
                        stderr
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        "diskutil returned non-zero status with no output".to_string()
                    }
                ));
            }

            return Ok(format!(
                "Native destructive erase completed via diskutil for device {} ({})",
                device.model, device.id
            ));
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = device;
            Err(
                "Native erase fallback is not implemented for this operating system. Set SECUREWIPE_ERASE_EXECUTABLE to override."
                    .to_string(),
            )
        }
    }
}

#[cfg(feature = "real_erase")]
pub fn build_offline_executor_from_env() -> Result<Box<dyn OfflineWipeExecutor + Send + Sync>, String> {
    if std::env::var("SECUREWIPE_DISABLE_NATIVE_ERASE_FALLBACK")
        .map(|v| v.trim().eq_ignore_ascii_case("1") || v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Ok(Box::new(CommandOfflineWipeExecutor::from_env()?));
    }

    if let Ok(command_executor) = CommandOfflineWipeExecutor::from_env() {
        return Ok(Box::new(command_executor));
    }

    Ok(Box::new(NativeOfflineWipeExecutor))
}

#[cfg(feature = "real_erase")]
fn apply_placeholders(template: &str, device: &Device) -> String {
    template
        .replace("{device_id}", &device.id)
        .replace("{device_model}", &device.model)
        .replace("{device_size_gb}", &device.size_gb.to_string())
        .replace(
            "{device_serial}",
            device.serial.as_deref().unwrap_or("unknown_serial"),
        )
}

#[cfg(feature = "real_erase")]
fn sanitize_for_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

#[cfg(feature = "real_erase")]
fn parse_windows_disk_number(device_id: &str) -> Option<u32> {
    let upper = device_id.trim().to_ascii_uppercase();
    if let Some(idx) = upper.find("PHYSICALDRIVE") {
        return upper[idx + "PHYSICALDRIVE".len()..].trim().parse::<u32>().ok();
    }
    if let Some(stripped) = upper.strip_prefix("DISK") {
        return stripped.trim().parse::<u32>().ok();
    }
    None
}

#[cfg(all(test, feature = "real_erase"))]
mod tests {
    use super::{
        apply_placeholders, build_offline_executor_from_env, parse_windows_disk_number,
        CommandOfflineWipeExecutor,
    };
    use crate::devices::{Device, DeviceDetectionConfidence};
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn sample_device() -> Device {
        Device {
            id: "disk9".to_string(),
            dev_type: "SSD".to_string(),
            model: "ModelZ".to_string(),
            serial: Some("SER-123".to_string()),
            size_gb: 512,
            allocated_gb: Some(128),
            partitions: vec![],
            connection: Some("SATA".to_string()),
            removable: Some(false),
            is_system: Some(false),
            smart_status: Some("OK".to_string()),
            temperature_c: Some(30.0),
            encrypted: false,
            hpa_dco: false,
            firmware: Some("FW1".to_string()),
            error: None,
            metadata: HashMap::new(),
            detection_confidence: DeviceDetectionConfidence::default(),
        }
    }

    #[test]
    fn placeholder_resolution_replaces_known_tokens() {
        let device = sample_device();
        let resolved = apply_placeholders(
            "wipe {device_id} {device_model} {device_size_gb} {device_serial}",
            &device,
        );
        assert!(resolved.contains("disk9"));
        assert!(resolved.contains("ModelZ"));
        assert!(resolved.contains("512"));
        assert!(resolved.contains("SER-123"));
    }

    #[test]
    fn from_env_rejects_missing_executable() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

        unsafe {
            std::env::remove_var("SECUREWIPE_ERASE_EXECUTABLE");
            std::env::remove_var("SECUREWIPE_ERASE_ARGS_JSON");
        }

        let result = CommandOfflineWipeExecutor::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn from_env_rejects_invalid_args_json() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

        unsafe {
            std::env::set_var("SECUREWIPE_ERASE_EXECUTABLE", "echo");
            std::env::set_var("SECUREWIPE_ERASE_ARGS_JSON", "not-json");
        }

        let result = CommandOfflineWipeExecutor::from_env();
        assert!(result.is_err());

        unsafe {
            std::env::remove_var("SECUREWIPE_ERASE_EXECUTABLE");
            std::env::remove_var("SECUREWIPE_ERASE_ARGS_JSON");
        }
    }

    #[test]
    fn parse_windows_disk_number_supports_physicaldrive_and_disk_prefix() {
        assert_eq!(parse_windows_disk_number("\\\\.\\PHYSICALDRIVE7"), Some(7));
        assert_eq!(parse_windows_disk_number("disk12"), Some(12));
        assert_eq!(parse_windows_disk_number("unknown"), None);
    }

    #[test]
    fn build_executor_uses_native_fallback_when_custom_env_missing() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

        unsafe {
            std::env::remove_var("SECUREWIPE_ERASE_EXECUTABLE");
            std::env::remove_var("SECUREWIPE_ERASE_ARGS_JSON");
            std::env::remove_var("SECUREWIPE_DISABLE_NATIVE_ERASE_FALLBACK");
        }
        let result = build_offline_executor_from_env();
        assert!(result.is_ok());
    }
}
