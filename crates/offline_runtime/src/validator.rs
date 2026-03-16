use crate::manifest::WipeManifest;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Validates the manifest fields and snapshot parity.
/// Returns `Err(reason)` if any guard fails.
pub fn validate_manifest(manifest: &WipeManifest) -> Result<(), String> {
    // Schema version parity
    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {}; expected {}",
            manifest.schema_version, SUPPORTED_SCHEMA_VERSION
        ));
    }

    // Required identity fields
    if manifest.session_id.trim().is_empty() {
        return Err("session_id is empty".to_string());
    }
    if manifest.target_device_id.trim().is_empty() {
        return Err("target_device_id is empty".to_string());
    }
    if manifest.target_device_model.trim().is_empty() {
        return Err("target_device_model is empty".to_string());
    }
    if manifest.target_device_size_gb == 0 {
        return Err("target_device_size_gb must be > 0".to_string());
    }

    // Phase must be usb_prepared or reboot_pending to allow execution
    let phase = manifest.phase.as_str();
    if !matches!(phase, "usb_prepared" | "reboot_pending" | "offline_started") {
        return Err(format!(
            "manifest phase '{}' is not eligible for offline execution; \
             expected usb_prepared, reboot_pending, or offline_started",
            phase
        ));
    }

    // Stage-1 snapshot hash parity
    if let (Some(snapshot), Some(recorded_hash)) = (
        &manifest.target_detection_snapshot,
        &manifest.target_detection_snapshot_sha256,
    ) {
        let computed = compute_snapshot_sha256(snapshot);
        if computed != *recorded_hash {
            return Err(format!(
                "stage-1 snapshot hash mismatch: recorded={} computed={}",
                recorded_hash, computed
            ));
        }
        println!("[offline_runtime] Stage-1 snapshot hash verified: {}", computed);
    } else if manifest.target_detection_snapshot_sha256.is_some() {
        return Err("snapshot hash recorded but snapshot payload is missing".to_string());
    }

    Ok(())
}

fn compute_snapshot_sha256(snapshot: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let canonical = snapshot.to_string();
    let hash = Sha256::digest(canonical.as_bytes());
    // encode as lowercase hex
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}
