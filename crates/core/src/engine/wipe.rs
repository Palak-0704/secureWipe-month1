//! src/engine/wipe.rs
//!
//! Wipe execution engine.
//! - In-app path is always simulation-only.
//! - Destructive operations are only allowed from offline runtime with explicit flags.

use crate::devices::Device;
#[cfg(feature = "real_erase")]
use crate::engine::offline_executor::build_offline_executor_from_env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeMode {
    Simulation,
    Destructive,
}

#[derive(Debug, Clone)]
pub struct WipeResult {
    pub mode: WipeMode,
    pub message: String,
}

#[cfg(feature = "real_erase")]
fn is_truthy_env(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v.trim().eq_ignore_ascii_case("1") || v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(feature = "real_erase")]
fn strict_targeting_enabled() -> bool {
    std::env::var("SECUREWIPE_STRICT_TARGETING")
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            !matches!(norm.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true)
}

#[cfg(feature = "real_erase")]
fn target_allowlist_ids() -> Vec<String> {
    std::env::var("SECUREWIPE_TARGET_ALLOWLIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg_attr(not(feature = "real_erase"), allow(dead_code))]
fn device_meets_strict_targeting(device: &Device, allowlist: &[String]) -> bool {
    if device.removable.unwrap_or(false) {
        return true;
    }

    allowlist.iter().any(|id| id == &device.id)
}

#[cfg(feature = "real_erase")]
fn validate_offline_destructive_guards() -> Result<(), String> {
    let runtime = std::env::var("SECUREWIPE_RUNTIME").unwrap_or_default();
    if runtime.trim().to_lowercase() != "offline" {
        return Err("Destructive wipe blocked: SECUREWIPE_RUNTIME must be 'offline'.".to_string());
    }

    if !is_truthy_env("ENABLE_REAL_ERASE") {
        return Err("Destructive wipe blocked: ENABLE_REAL_ERASE=1 is required.".to_string());
    }

    if !is_truthy_env("SECUREWIPE_OFFLINE_CONFIRMED") {
        return Err(
            "Destructive wipe blocked: SECUREWIPE_OFFLINE_CONFIRMED=1 is required.".to_string(),
        );
    }

    Ok(())
}

/// Always-safe in-app wipe path.
/// This must never perform destructive operations on a live host runtime.
pub fn perform_wipe_in_app(device: &Device) -> WipeResult {
    WipeResult {
        mode: WipeMode::Simulation,
        message: format!(
            "Simulated in-app wipe performed on device: {} ({})",
            device.model, device.id
        ),
    }
}

/// Offline-only destructive wipe path.
///
/// This function is intentionally gated by:
/// - compile-time feature: `real_erase`
/// - runtime mode: `SECUREWIPE_RUNTIME=offline`
/// - explicit flags: `ENABLE_REAL_ERASE=1`, `SECUREWIPE_OFFLINE_CONFIRMED=1`
pub fn perform_wipe_offline(device: &Device) -> Result<WipeResult, String> {
    #[cfg(not(feature = "real_erase"))]
    {
        let _ = device;
        return Err(
            "Destructive wipe blocked: binary not built with real_erase feature.".to_string(),
        );
    }

    #[cfg(feature = "real_erase")]
    {
        if device.is_system.unwrap_or(false) {
            return Err(
                "Destructive wipe blocked: target device is marked as a protected system disk."
                    .to_string(),
            );
        }

        if strict_targeting_enabled() {
            let allowlist = target_allowlist_ids();
            if !device_meets_strict_targeting(device, &allowlist) {
                return Err(
                    "Destructive wipe blocked: strict targeting allows only removable devices or allowlisted IDs."
                        .to_string(),
                );
            }
        }

        validate_offline_destructive_guards()?;
        let executor = build_offline_executor_from_env()?;
        let message = executor.execute(device)?;

        Ok(WipeResult {
            mode: WipeMode::Destructive,
            message,
        })
    }
}

pub fn perform_wipe(device: &Device) -> String {
    perform_wipe_in_app(device).message
}

#[cfg(test)]
mod tests {
    use super::{device_meets_strict_targeting, perform_wipe_in_app, perform_wipe_offline, WipeMode};
    use crate::devices::{Device, DeviceDetectionConfidence};
    use std::collections::HashMap;

    fn sample_device() -> Device {
        Device {
            id: "disk0".to_string(),
            dev_type: "SSD".to_string(),
            model: "SampleSSD".to_string(),
            serial: Some("SER123".to_string()),
            size_gb: 512,
            allocated_gb: Some(128),
            partitions: vec![],
            connection: Some("SATA".to_string()),
            removable: Some(false),
            is_system: Some(true),
            smart_status: Some("OK".to_string()),
            temperature_c: Some(35.0),
            encrypted: false,
            hpa_dco: false,
            firmware: Some("FW1.0".to_string()),
            error: None,
            metadata: HashMap::new(),
            detection_confidence: DeviceDetectionConfidence::default(),
        }
    }

    #[test]
    fn in_app_path_is_always_simulation() {
        let result = perform_wipe_in_app(&sample_device());
        assert_eq!(result.mode, WipeMode::Simulation);
        assert!(result.message.to_lowercase().contains("simulated"));
    }

    #[test]
    fn offline_path_requires_guards_or_is_blocked() {
        // Ensure clean test environment for guard vars.
        unsafe {
            std::env::remove_var("SECUREWIPE_RUNTIME");
            std::env::remove_var("ENABLE_REAL_ERASE");
            std::env::remove_var("SECUREWIPE_OFFLINE_CONFIRMED");
        }

        let result = perform_wipe_offline(&sample_device());
        assert!(result.is_err());
    }

    #[cfg(feature = "real_erase")]
    #[test]
    fn offline_path_blocks_protected_system_disks() {
        let result = perform_wipe_offline(&sample_device());
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap_or_default()
            .contains("protected system disk"));
    }

    #[test]
    fn strict_targeting_predicate_allows_removable_or_allowlisted() {
        let non_removable = sample_device();
        let removable = Device {
            removable: Some(true),
            ..sample_device()
        };
        let allowlist = vec!["disk0".to_string()];
        let empty = Vec::<String>::new();

        assert!(device_meets_strict_targeting(&removable, &empty));
        assert!(device_meets_strict_targeting(&non_removable, &allowlist));
        assert!(!device_meets_strict_targeting(&non_removable, &empty));
    }
}
