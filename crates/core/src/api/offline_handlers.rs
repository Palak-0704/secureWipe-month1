use axum::{extract::Path, Json};
use chrono::Utc;
use serde_json::json;
use std::path::{Path as FsPath, PathBuf};

use crate::ai::{recommend_method, ComplianceContext};
use crate::devices::{detect_devices, DetectionConfidenceLevel};
use crate::engine::wipe::{perform_wipe_offline, WipeMode};

use super::errors::AppError;
use super::anomaly_detector::detect_offline_result_anomalies;
use super::result_policy::{completion_outcome_message, validate_offline_result_contract};
use super::usb_imaging::{run_usb_provisioning, UsbProvisionMode};
use super::storage::{
    append_history, data_path, new_operation_id, now_id, read_session_manifest, session_manifest_path,
    write_offline_result, write_session_manifest,
};
use super::types::{
    default_offline_result_schema_version, default_wipe_manifest_schema_version,
    CreateWipeSessionRequest, CreateWipeSessionResponse, HistoryEntry, OfflineExecuteRequest,
    OfflineExecuteResponse, OfflineResultIngestRequest, OfflineResultIngestResponse,
    OfflineResultRecord,
    PrepareUsbRequest, PrepareUsbResponse, ResumeAction, ResumeSessionResponse,
    SessionStatusResponse, UsbCandidateResponse, WipeSessionManifest, WipeSessionPhase,
};

fn ensure_supported_manifest_schema(manifest: &WipeSessionManifest) -> Result<(), AppError> {
    let expected = default_wipe_manifest_schema_version();
    if manifest.schema_version != expected {
        return Err(AppError::conflict(
            "unsupported_session_schema_version",
            format!(
                "Session manifest schema version {} is unsupported; expected version {}.",
                manifest.schema_version, expected
            ),
        ));
    }

    Ok(())
}

fn ensure_supported_result_schema(schema_version: u32) -> Result<(), AppError> {
    let expected = default_offline_result_schema_version();
    if schema_version != expected {
        return Err(AppError::conflict(
            "unsupported_offline_result_schema_version",
            format!(
                "Offline result schema version {} is unsupported; expected version {}.",
                schema_version, expected
            ),
        ));
    }

    Ok(())
}

fn manifest_matches_device(manifest: &WipeSessionManifest, device: &crate::devices::Device) -> bool {
    if manifest.target_device_id != device.id {
        return false;
    }
    if manifest.target_device_model != device.model {
        return false;
    }
    if manifest.target_device_size_gb != device.size_gb {
        return false;
    }

    match &manifest.target_device_serial {
        Some(serial) => device.serial.as_deref() == Some(serial.as_str()),
        None => true,
    }
}

fn build_detection_snapshot(device: &crate::devices::Device) -> serde_json::Value {
    let partitions = device
        .partitions
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "mount_point": p.mount_point,
                "size_gb": p.size_gb,
                "used_gb": p.used_gb,
                "fs_type": p.fs_type,
                "is_system": p.is_system,
                "is_boot": p.is_boot,
                "encrypted": p.encrypted
            })
        })
        .collect::<Vec<_>>();

    json!({
        "device_id": device.id,
        "model": device.model,
        "size_gb": device.size_gb,
        "serial": device.serial,
        "connection": device.connection,
        "removable": device.removable,
        "is_system": device.is_system,
        "encrypted": device.encrypted,
        "hpa_dco": device.hpa_dco,
        "detection_confidence": device.detection_confidence,
        "partitions": partitions
    })
}

fn detect_devices_for_flow() -> Vec<crate::devices::Device> {
    if let Ok(path) = std::env::var("SECUREWIPE_DEVICE_FIXTURE_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            if let Ok(raw) = std::fs::read_to_string(trimmed) {
                if let Ok(parsed) = serde_json::from_str::<Vec<crate::devices::Device>>(&raw) {
                    return parsed;
                }
            }
        }
    }

    detect_devices()
}

fn offline_runtime_binary_name() -> &'static str {
    if cfg!(windows) {
        "offline_runtime.exe"
    } else {
        "offline_runtime"
    }
}

fn discover_offline_runtime_binary() -> Option<PathBuf> {
    if let Ok(configured) = std::env::var("SECUREWIPE_OFFLINE_RUNTIME_BINARY") {
        let path = PathBuf::from(configured.trim());
        if path.is_file() {
            return Some(path);
        }
    }

    if parse_env_bool("SECUREWIPE_DISABLE_OFFLINE_RUNTIME_AUTO_DISCOVERY", false) {
        return None;
    }

    let binary_name = offline_runtime_binary_name();
    let mut candidates = vec![
        PathBuf::from("target").join("debug").join(binary_name),
        PathBuf::from("target").join("release").join(binary_name),
    ];

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(binary_name));
            if parent.ends_with("deps") {
                if let Some(debug_dir) = parent.parent() {
                    candidates.push(debug_dir.join(binary_name));
                }
            }
        }
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn bundle_offline_runtime_artifacts(output_dir: &FsPath) -> Result<bool, AppError> {
    let binary_name = offline_runtime_binary_name();
    let binary_path = output_dir.join(binary_name);
    let status_path = output_dir.join("OFFLINE_RUNTIME_STATUS.txt");

    if let Some(source_path) = discover_offline_runtime_binary() {
        std::fs::copy(&source_path, &binary_path).map_err(|_| {
            AppError::internal_server_error(
                "offline_runtime_bundle_failed",
                format!(
                    "Failed to copy offline runtime binary from {} into handoff package.",
                    source_path.display()
                ),
            )
        })?;

        let status = format!(
            "bundled\nsource={}\ntarget={}\n",
            source_path.display(),
            binary_path.display()
        );
        let _ = std::fs::write(status_path, status);
        return Ok(true);
    }

    let status = format!(
        "missing\nexpected_binary={}\nhint=Build with `cargo build -p securewipe-cli --bin offline_runtime` or set SECUREWIPE_OFFLINE_RUNTIME_BINARY to an absolute binary path before preparing USB.\n",
        binary_name
    );
    let _ = std::fs::write(status_path, status);
    Ok(false)
}

fn device_is_protected_target(device: &crate::devices::Device) -> bool {
    device.is_system.unwrap_or(false)
}

fn strict_targeting_enabled() -> bool {
    std::env::var("SECUREWIPE_STRICT_TARGETING")
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            !matches!(norm.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true)
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            matches!(norm.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}

fn parse_env_u64(name: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn log_operation_event(event: &str, session_id: Option<&str>, operation_id: Option<&str>, detail: &str) {
    println!(
        "[OPERATION] event={} session_id={} operation_id={} detail={}",
        event,
        session_id.unwrap_or("-"),
        operation_id.unwrap_or("-"),
        detail
    );
}

fn usb_provision_mode() -> UsbProvisionMode {
    match std::env::var("SECUREWIPE_USB_PROVISION_MODE") {
        Ok(v) if v.trim().eq_ignore_ascii_case("real") => UsbProvisionMode::Real,
        _ => UsbProvisionMode::Simulation,
    }
}

fn min_usb_size_gb() -> u64 {
    parse_env_u64("SECUREWIPE_USB_MIN_SIZE_GB", 8, 1, 4096)
}

fn usb_require_overwrite_confirmation() -> bool {
    parse_env_bool("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION", true)
}

fn usb_real_breakglass_enabled() -> bool {
    parse_env_bool("SECUREWIPE_USB_REAL_BREAKGLASS", false)
}

fn usb_overwrite_confirmation_valid(value: Option<&str>) -> bool {
    value
        .map(|v| v.trim().eq_ignore_ascii_case("ERASE_USB"))
        .unwrap_or(false)
}

fn usb_real_allowlist_ids() -> Vec<String> {
    std::env::var("SECUREWIPE_USB_REAL_ALLOWLIST")
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

fn ensure_usb_real_allowlisted(mode: UsbProvisionMode, usb_device_id: &str) -> Result<(), AppError> {
    if mode != UsbProvisionMode::Real {
        return Ok(());
    }

    let allowlist = usb_real_allowlist_ids();
    if allowlist.is_empty() {
        return Err(AppError::forbidden(
            "usb_real_allowlist_required",
            "Real USB provisioning requires SECUREWIPE_USB_REAL_ALLOWLIST to explicitly list allowed removable device IDs.",
        ));
    }

    if !allowlist.iter().any(|id| id == usb_device_id) {
        return Err(AppError::forbidden(
            "usb_device_not_allowlisted_for_real_provision",
            "Requested USB device is not explicitly allowlisted for real provisioning.",
        ));
    }

    Ok(())
}

fn ensure_usb_real_breakglass(mode: UsbProvisionMode) -> Result<(), AppError> {
    if mode != UsbProvisionMode::Real {
        return Ok(());
    }

    if !usb_real_breakglass_enabled() {
        return Err(AppError::forbidden(
            "usb_real_breakglass_required",
            "Real USB provisioning is blocked by default. Set SECUREWIPE_USB_REAL_BREAKGLASS=1 only for controlled lab usage.",
        ));
    }

    Ok(())
}

fn allow_unknown_detection_confidence() -> bool {
    parse_env_bool("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", false)
        || parse_env_bool("SECUREWIPE_ALLOW_UNKNOWN_DETECTION", false)
}

fn device_detection_confidence_sufficient(device: &crate::devices::Device) -> bool {
    device.detection_confidence.is_system != DetectionConfidenceLevel::Unknown
        && device.detection_confidence.encrypted != DetectionConfidenceLevel::Unknown
        && device.detection_confidence.hpa_dco != DetectionConfidenceLevel::Unknown
}

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

fn device_meets_strict_targeting(device: &crate::devices::Device, allowlist: &[String]) -> bool {
    if device.removable.unwrap_or(false) {
        return true;
    }

    allowlist.iter().any(|id| id == &device.id)
}

fn ensure_usb_candidate_suitable(device: &crate::devices::Device) -> Result<(), AppError> {
    if device.removable != Some(true) && device.dev_type.to_uppercase() != "USB" {
        return Err(AppError::forbidden(
            "usb_device_not_removable",
            "Requested target is not removable/USB and cannot be used for USB provisioning.",
        ));
    }

    if device.size_gb < min_usb_size_gb() {
        return Err(AppError::unprocessable_entity(
            "usb_device_size_insufficient",
            format!(
                "USB device size {} GB is below required minimum {} GB.",
                device.size_gb,
                min_usb_size_gb()
            ),
        ));
    }

    Ok(())
}

fn ensure_device_target_permitted(device: &crate::devices::Device) -> Result<(), AppError> {
    if device_is_protected_target(device) {
        return Err(AppError::forbidden(
            "protected_system_device",
            "Target device is marked as a protected system disk and cannot be wiped.",
        ));
    }

    if strict_targeting_enabled() {
        let allowlist = target_allowlist_ids();
        if !device_meets_strict_targeting(device, &allowlist) {
            return Err(AppError::forbidden(
                "target_device_not_permitted",
                "Strict targeting is enabled: only removable devices or allowlisted IDs are permitted for destructive wipe flows.",
            ));
        }
    }

    Ok(())
}

pub async fn get_session_status(
    Path(session_id): Path<String>,
) -> Result<Json<SessionStatusResponse>, AppError> {
    let Some(manifest) = read_session_manifest(&session_id) else {
        return Err(AppError::not_found(
            "session_manifest_not_found",
            "Session manifest not found.",
        ));
    };

    Ok(Json(SessionStatusResponse {
        status: "session_status".to_string(),
        session_id,
        phase: manifest.phase,
        progress_percent: manifest.progress_percent,
        resume_required: manifest.resume_required,
        resume_hint: manifest.resume_hint,
    }))
}

pub async fn resume_wipe_session(
    Path(session_id): Path<String>,
) -> Result<Json<ResumeSessionResponse>, AppError> {
    let Some(mut manifest) = read_session_manifest(&session_id) else {
        return Err(AppError::not_found(
            "session_manifest_not_found",
            "Session manifest not found.",
        ));
    };

    ensure_supported_manifest_schema(&manifest)?;

    let (recommended_action, message) = match manifest.phase {
        WipeSessionPhase::InAppPrepared => {
            manifest.resume_required = true;
            manifest.resume_hint = Some("prepare_bootable_usb".to_string());
            (
                ResumeAction::PrepareUsb,
                "Prepare bootable USB before continuing the offline wipe flow.".to_string(),
            )
        }
        WipeSessionPhase::UsbPrepared => {
            manifest.phase = WipeSessionPhase::RebootPending;
            manifest.resume_required = true;
            manifest.resume_hint = Some("boot_into_offline_environment".to_string());
            (
                ResumeAction::RebootToOffline,
                "Resume by rebooting into the prepared offline environment.".to_string(),
            )
        }
        WipeSessionPhase::RebootPending | WipeSessionPhase::OfflineStarted | WipeSessionPhase::Wiping => {
            manifest.resume_required = true;
            manifest.resume_hint = Some("await_offline_result_ingest".to_string());
            (
                ResumeAction::AwaitOfflineResultIngest,
                "Await offline wipe completion and ingest the result manifest.".to_string(),
            )
        }
        WipeSessionPhase::Verified | WipeSessionPhase::Certified | WipeSessionPhase::Completed => {
            manifest.resume_required = false;
            manifest.resume_hint = None;
            (
                ResumeAction::ReviewCompletion,
                "Session is already past destructive execution; review completion artifacts.".to_string(),
            )
        }
        WipeSessionPhase::Failed => {
            return Err(AppError::conflict(
                "session_resume_blocked_failed",
                "Failed sessions require manual intervention before any resume action.",
            ));
        }
    };

    write_session_manifest(&manifest);

    Ok(Json(ResumeSessionResponse {
        status: "session_resume_ready".to_string(),
        session_id,
        phase: manifest.phase,
        progress_percent: manifest.progress_percent,
        resume_required: manifest.resume_required,
        recommended_action,
        resume_hint: manifest.resume_hint,
        message,
    }))
}

pub async fn execute_offline_wipe(
    Json(req): Json<OfflineExecuteRequest>,
) -> Result<Json<OfflineExecuteResponse>, AppError> {
    log_operation_event(
        "offline_execute_requested",
        Some(&req.session_id),
        None,
        "offline wipe execution request received",
    );

    if req.confirmation_text.trim().to_uppercase() != "ERASE" {
        return Err(AppError::unprocessable_entity(
            "offline_confirmation_invalid",
            "Offline destructive wipe requires confirmation text ERASE.",
        ));
    }

    let Some(mut manifest) = read_session_manifest(&req.session_id) else {
        return Err(AppError::not_found(
            "session_manifest_not_found",
            "Offline wipe manifest not found.",
        ));
    };

    ensure_supported_manifest_schema(&manifest)?;

    if !manifest.phase.can_transition_to(&WipeSessionPhase::OfflineStarted) {
        return Err(AppError::conflict(
            "invalid_session_phase_for_offline_start",
            "Session is not ready to start offline wipe.",
        ));
    }

    manifest.phase = WipeSessionPhase::OfflineStarted;
    manifest.progress_percent = manifest.progress_percent.max(40);
    manifest.resume_required = true;
    manifest.resume_hint = Some("awaiting_offline_result_ingest".to_string());
    write_session_manifest(&manifest);

    let devices = detect_devices_for_flow();
    let Some(device) = devices.iter().find(|d| d.id == manifest.target_device_id) else {
        return Err(AppError::not_found(
            "target_device_not_found",
            "Target device from manifest not found in runtime scan.",
        ));
    };

    ensure_device_target_permitted(device)?;

    let confidence_sufficient = device_detection_confidence_sufficient(device);
    if !confidence_sufficient && !allow_unknown_detection_confidence() {
        append_history(HistoryEntry {
            device_id: device.id.clone(),
            model: device.model.clone(),
            recommendation: manifest.method.clone(),
            explanation: "Offline destructive wipe blocked: critical device detection confidence is unknown. Set SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE=1 only for controlled emergency workflows.".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            operation_id: Some(new_operation_id()),
            wipe_id: Some(manifest.session_id.clone()),
            phase: Some("offline_destructive_blocked_confidence".to_string()),
        });

        return Err(AppError::forbidden(
            "detection_confidence_insufficient",
            "Offline destructive wipe requires known detection confidence for critical fields (is_system, encrypted, hpa_dco).",
        ));
    }

    if !confidence_sufficient && allow_unknown_detection_confidence() {
        append_history(HistoryEntry {
            device_id: device.id.clone(),
            model: device.model.clone(),
            recommendation: manifest.method.clone(),
            explanation: "Override active: proceeding with offline destructive flow despite unknown detection confidence (SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE=1).".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            operation_id: Some(new_operation_id()),
            wipe_id: Some(manifest.session_id.clone()),
            phase: Some("offline_destructive_confidence_override".to_string()),
        });
    }

    if !manifest_matches_device(&manifest, device) {
        return Err(AppError::conflict(
            "offline_target_identity_mismatch",
            "Offline runtime device identity does not match session manifest target identity.",
        ));
    }

    if manifest.target_detection_snapshot.is_some()
        || manifest.target_detection_snapshot_sha256.is_some()
        || manifest.target_detection_snapshot_signature.is_some()
    {
        let Some(stage1_snapshot) = manifest.target_detection_snapshot.as_ref() else {
            return Err(AppError::conflict(
                "offline_stage1_snapshot_missing",
                "Session manifest is missing the recorded stage-1 detection snapshot.",
            ));
        };

        let Some(stage1_snapshot_sha256) = manifest.target_detection_snapshot_sha256.as_ref() else {
            return Err(AppError::conflict(
                "offline_stage1_snapshot_hash_missing",
                "Session manifest is missing the recorded stage-1 detection snapshot hash.",
            ));
        };

        let recomputed_stage1_sha = super::certificate_crypto::payload_sha256(stage1_snapshot)?;
        if recomputed_stage1_sha != *stage1_snapshot_sha256 {
            return Err(AppError::conflict(
                "offline_stage1_snapshot_tampered",
                "Recorded stage-1 detection snapshot hash does not match its payload.",
            ));
        }

        if let Some(signature) = manifest.target_detection_snapshot_signature.as_ref() {
            let verified = super::certificate_crypto::verify_certificate_payload(
                stage1_snapshot,
                &signature.signature_base64,
                &signature.public_key_base64,
            )?;
            if !verified {
                return Err(AppError::conflict(
                    "offline_stage1_snapshot_signature_invalid",
                    "Recorded stage-1 detection snapshot signature is invalid.",
                ));
            }

            if signature.payload_sha256 != *stage1_snapshot_sha256 {
                return Err(AppError::conflict(
                    "offline_stage1_snapshot_signature_hash_mismatch",
                    "Recorded stage-1 detection snapshot signature hash does not match the manifest hash.",
                ));
            }
        }

        let stage2_snapshot = build_detection_snapshot(device);
        let stage2_snapshot_sha256 = super::certificate_crypto::payload_sha256(&stage2_snapshot)?;
        if stage2_snapshot_sha256 != *stage1_snapshot_sha256 {
            append_history(HistoryEntry {
                device_id: device.id.clone(),
                model: device.model.clone(),
                recommendation: manifest.method.clone(),
                explanation: "Offline destructive wipe blocked: stage-2 device rescan does not match the signed stage-1 detection snapshot.".to_string(),
                timestamp: Utc::now().to_rfc3339(),
                operation_id: Some(new_operation_id()),
                wipe_id: Some(manifest.session_id.clone()),
                phase: Some("offline_stage2_snapshot_mismatch".to_string()),
            });

            return Err(AppError::conflict(
                "offline_stage2_detection_mismatch",
                "Offline stage-2 device rescan does not match the recorded stage-1 detection snapshot.",
            ));
        }
    }

    match perform_wipe_offline(device) {
        Ok(result) => {
            manifest.phase = WipeSessionPhase::Wiping;
            manifest.progress_percent = manifest.progress_percent.max(70);
            write_session_manifest(&manifest);

            let operation_id = new_operation_id();
            log_operation_event(
                "offline_execute_started",
                Some(&manifest.session_id),
                Some(&operation_id),
                &result.message,
            );

            append_history(HistoryEntry {
                device_id: device.id.clone(),
                model: device.model.clone(),
                recommendation: manifest.method.clone(),
                explanation: result.message.clone(),
                timestamp: Utc::now().to_rfc3339(),
                operation_id: Some(operation_id),
                wipe_id: Some(manifest.session_id.clone()),
                phase: Some("offline_destructive".to_string()),
            });
            Ok(Json(OfflineExecuteResponse {
                status: "offline_wipe_started".to_string(),
                session_id: manifest.session_id,
                phase: WipeSessionPhase::Wiping,
                progress_percent: manifest.progress_percent,
                resume_required: manifest.resume_required,
                resume_hint: manifest.resume_hint.clone(),
                mode: match result.mode {
                    WipeMode::Simulation => "simulation".to_string(),
                    WipeMode::Destructive => "destructive".to_string(),
                },
                message: result.message,
            }))
        }
        Err(e) => {
            manifest.phase = WipeSessionPhase::Failed;
            manifest.progress_percent = 100;
            manifest.resume_required = false;
            manifest.resume_hint = None;
            write_session_manifest(&manifest);
            log_operation_event(
                "offline_execute_blocked",
                Some(&manifest.session_id),
                None,
                &e,
            );
            Err(AppError::forbidden("offline_wipe_blocked", e))
        }
    }
}

pub async fn ingest_offline_result(
    Json(req): Json<OfflineResultIngestRequest>,
) -> Result<Json<OfflineResultIngestResponse>, AppError> {
    log_operation_event(
        "offline_result_ingest_requested",
        Some(&req.session_id),
        None,
        "offline result ingest request received",
    );

    ensure_supported_result_schema(req.schema_version)?;

    if req.session_id.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "session_id_required",
            "Session ID is required.",
        ));
    }
    if req.target_device_id.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "target_device_id_required",
            "Target device ID is required.",
        ));
    }
    if req.target_device_model.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "target_device_model_required",
            "Target device model is required.",
        ));
    }
    if req.target_device_size_gb == 0 {
        return Err(AppError::unprocessable_entity(
            "target_device_size_required",
            "Target device size must be greater than zero.",
        ));
    }
    validate_offline_result_contract(&req)?;

    let Some(mut manifest) = read_session_manifest(&req.session_id) else {
        return Err(AppError::not_found(
            "session_manifest_not_found",
            "Cannot ingest offline result because session manifest was not found.",
        ));
    };

    ensure_supported_manifest_schema(&manifest)?;

    if manifest.target_device_id != req.target_device_id
        || manifest.target_device_model != req.target_device_model
        || manifest.target_device_size_gb != req.target_device_size_gb
    {
        return Err(AppError::conflict(
            "offline_result_identity_mismatch",
            "Offline result target identity does not match session manifest target identity.",
        ));
    }

    if !manifest.phase.can_transition_to(&WipeSessionPhase::Verified) {
        return Err(AppError::conflict(
            "invalid_session_phase_for_result_ingest",
            "Session is not in a phase that can accept offline results.",
        ));
    }

    let anomalies = detect_offline_result_anomalies(
        req.verification_passed,
        &req.completion_status,
        req.verification_notes.as_deref(),
        req.verification_evidence.as_ref(),
    );
    if !anomalies.is_empty() {
        manifest.phase = WipeSessionPhase::Failed;
        manifest.progress_percent = manifest.progress_percent.max(90);
        manifest.resume_required = true;
        manifest.resume_hint = Some("manual_anomaly_review_required".to_string());
        write_session_manifest(&manifest);

        let operation_id = new_operation_id();
        log_operation_event(
            "offline_result_ingest_anomaly_paused",
            Some(&req.session_id),
            Some(&operation_id),
            &anomalies.join(" | "),
        );

        append_history(HistoryEntry {
            device_id: manifest.target_device_id.clone(),
            model: manifest.target_device_model.clone(),
            recommendation: manifest.method.clone(),
            explanation: format!(
                "Offline result ingest paused by anomaly detector: {}",
                anomalies.join(" | ")
            ),
            timestamp: Utc::now().to_rfc3339(),
            operation_id: Some(operation_id),
            wipe_id: Some(req.session_id.clone()),
            phase: Some("offline_result_anomaly_paused".to_string()),
        });

        return Err(AppError::conflict(
            "offline_wipe_anomaly_detected",
            format!(
                "Anomaly detector paused result ingest for manual review: {}",
                anomalies.join(" | ")
            ),
        ));
    }

    manifest.phase = WipeSessionPhase::Verified;
    manifest.progress_percent = manifest.progress_percent.max(90);
    write_session_manifest(&manifest);

    let result_record = OfflineResultRecord {
        schema_version: req.schema_version,
        session_id: req.session_id.clone(),
        target_device_id: req.target_device_id.clone(),
        target_device_model: req.target_device_model.clone(),
        target_device_size_gb: req.target_device_size_gb,
        verification_passed: req.verification_passed,
        verification_notes: req.verification_notes.clone(),
        completion_status: req.completion_status.clone(),
        verification_evidence: req.verification_evidence.clone(),
        ingested_at: Utc::now().to_rfc3339(),
    };
    write_offline_result(&result_record);

    if req.verification_passed && req.completion_status.is_verified() {
        manifest.phase = WipeSessionPhase::Completed;
    } else {
        manifest.phase = WipeSessionPhase::Failed;
    }
    manifest.progress_percent = 100;
    manifest.resume_required = false;
    manifest.resume_hint = None;
    write_session_manifest(&manifest);

    let operation_id = new_operation_id();
    log_operation_event(
        "offline_result_ingested",
        Some(&req.session_id),
        Some(&operation_id),
        "offline result reconciled with session manifest",
    );

    append_history(HistoryEntry {
        device_id: manifest.target_device_id.clone(),
        model: manifest.target_device_model.clone(),
        recommendation: manifest.method.clone(),
        explanation: completion_outcome_message(req.verification_passed, &req.completion_status),
        timestamp: Utc::now().to_rfc3339(),
        operation_id: Some(operation_id),
        wipe_id: Some(req.session_id.clone()),
        phase: Some("offline_result_ingested".to_string()),
    });

    Ok(Json(OfflineResultIngestResponse {
        status: "offline_result_ingested".to_string(),
        session_id: req.session_id,
        phase: manifest.phase,
        progress_percent: manifest.progress_percent,
        resume_required: manifest.resume_required,
        resume_hint: manifest.resume_hint,
        reconciled: true,
        message: "Offline result successfully reconciled with session manifest.".to_string(),
    }))
}

pub async fn list_wipe_sessions() -> Json<Vec<super::types::WipeSessionManifest>> {
    Json(super::storage::list_sessions())
}

pub async fn list_usb_candidates() -> Json<Vec<UsbCandidateResponse>> {
    let devices = detect_devices_for_flow();
    let usbs = devices
        .into_iter()
        .filter(|d| d.dev_type.to_uppercase() == "USB" || d.removable == Some(true))
        .map(|d| UsbCandidateResponse {
            id: d.id,
            model: d.model,
            size_gb: d.size_gb,
            removable: d.removable.unwrap_or(false),
        })
        .collect::<Vec<_>>();
    Json(usbs)
}

pub async fn create_wipe_session(
    Json(req): Json<CreateWipeSessionRequest>,
) -> Result<Json<CreateWipeSessionResponse>, AppError> {
    log_operation_event(
        "session_create_requested",
        None,
        None,
        "offline wipe session creation requested",
    );

    if req.mode.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "mode_required",
            "Session mode is required.",
        ));
    }

    if req.target_device_id.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "target_device_id_required",
            "Target device ID is required.",
        ));
    }

    let devices = detect_devices_for_flow();
    let Some(device) = devices.iter().find(|d| d.id == req.target_device_id) else {
        return Err(AppError::not_found(
            "target_device_not_found",
            "Target device for wipe session was not found.",
        ));
    };

    let confidence_sufficient = device_detection_confidence_sufficient(device);
    if !confidence_sufficient && !allow_unknown_detection_confidence() {
        append_history(HistoryEntry {
            device_id: device.id.clone(),
            model: device.model.clone(),
            recommendation: "session_create".to_string(),
            explanation: "Session creation blocked: critical device detection confidence is unknown. Set SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE=1 only for controlled emergency workflows.".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            operation_id: Some(new_operation_id()),
            wipe_id: None,
            phase: Some("session_create_blocked_confidence".to_string()),
        });

        return Err(AppError::forbidden(
            "detection_confidence_insufficient",
            "Wipe session creation requires known detection confidence for critical fields (is_system, encrypted, hpa_dco).",
        ));
    }

    if !confidence_sufficient && allow_unknown_detection_confidence() {
        append_history(HistoryEntry {
            device_id: device.id.clone(),
            model: device.model.clone(),
            recommendation: "session_create".to_string(),
            explanation: "Override active: creating session despite unknown detection confidence (SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE=1).".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            operation_id: Some(new_operation_id()),
            wipe_id: None,
            phase: Some("session_create_confidence_override".to_string()),
        });
    }

    ensure_device_target_permitted(device)?;

    let compliance = req.compliance.unwrap_or_default();
    let ctx = ComplianceContext {
        gdpr: compliance.to_lowercase().contains("gdpr"),
        hipaa: compliance.to_lowercase().contains("hipaa"),
        nist: compliance.to_lowercase().contains("nist"),
        custom: None,
    };
    let rec = recommend_method(device, Some(&ctx));
    let stage1_snapshot = build_detection_snapshot(device);
    let stage1_snapshot_sha256 = super::certificate_crypto::payload_sha256(&stage1_snapshot)?;
    let stage1_snapshot_signature = match super::certificate_crypto::sign_certificate_payload(&stage1_snapshot) {
        Ok(signature) => Some(signature),
        Err(err) if err.code == "certificate_signing_seed_missing" => None,
        Err(err) => return Err(err),
    };

    let session_id = now_id("session");
    let manifest = WipeSessionManifest {
        schema_version: default_wipe_manifest_schema_version(),
        session_id: session_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        mode: req.mode,
        target_device_id: device.id.clone(),
        target_device_model: device.model.clone(),
        target_device_size_gb: device.size_gb,
        target_device_serial: device.serial.clone(),
        target_detection_snapshot: Some(stage1_snapshot),
        target_detection_snapshot_sha256: Some(stage1_snapshot_sha256),
        target_detection_snapshot_signature: stage1_snapshot_signature,
        method: rec.method,
        estimated_minutes: rec.estimated_minutes,
        risk_level: rec.risk_level,
        final_confirmation_required: "ERASE".to_string(),
        phase: WipeSessionPhase::InAppPrepared,
        progress_percent: 10,
        resume_required: false,
        resume_hint: None,
    };

    write_session_manifest(&manifest);
    let manifest_path = session_manifest_path(&session_id);

    let operation_id = new_operation_id();
    log_operation_event(
        "session_created",
        Some(&session_id),
        Some(&operation_id),
        &format!("target_device_id={} method={}", device.id, manifest.method),
    );

    append_history(HistoryEntry {
        device_id: device.id.clone(),
        model: device.model.clone(),
        recommendation: manifest.method.clone(),
        explanation: "Offline wipe session manifest prepared.".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        operation_id: Some(operation_id),
        wipe_id: Some(session_id.clone()),
        phase: Some("in_app_prepared".to_string()),
    });

    Ok(Json(CreateWipeSessionResponse {
        status: "session_created".to_string(),
        session_id,
        manifest_path,
        phase: WipeSessionPhase::InAppPrepared,
        progress_percent: 10,
        resume_required: false,
        resume_hint: None,
    }))
}

pub async fn prepare_bootable_usb(
    Json(req): Json<PrepareUsbRequest>,
) -> Result<Json<PrepareUsbResponse>, AppError> {
    log_operation_event(
        "usb_prepare_requested",
        Some(&req.session_id),
        None,
        &format!("usb_device_id={}", req.usb_device_id),
    );

    if req.session_id.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "session_id_required",
            "Session ID is required.",
        ));
    }

    if req.usb_device_id.trim().is_empty() {
        return Err(AppError::unprocessable_entity(
            "usb_device_id_required",
            "USB device ID is required.",
        ));
    }

    let provisioning_mode = usb_provision_mode();
    if provisioning_mode == UsbProvisionMode::Real
        && usb_require_overwrite_confirmation()
        && !usb_overwrite_confirmation_valid(req.usb_overwrite_confirmation_text.as_deref())
    {
        return Err(AppError::unprocessable_entity(
            "usb_overwrite_confirmation_required",
            "Real USB provisioning requires usb_overwrite_confirmation_text=ERASE_USB.",
        ));
    }

    let Some(mut manifest) = read_session_manifest(&req.session_id) else {
        return Err(AppError::not_found(
            "session_manifest_not_found",
            "Session manifest not found. Create session before preparing USB.",
        ));
    };

    if !manifest.phase.can_transition_to(&WipeSessionPhase::UsbPrepared) {
        return Err(AppError::conflict(
            "invalid_session_phase_for_usb_prepare",
            "Session is not in a phase that can prepare bootable USB.",
        ));
    }

    let session_manifest_path_value = session_manifest_path(&req.session_id);
    let manifest_raw = std::fs::read_to_string(&session_manifest_path_value).map_err(|_| {
        AppError::not_found(
            "session_manifest_not_found",
            "Session manifest not found. Create session before preparing USB.",
        )
    })?;

    let usb_candidates = detect_devices_for_flow();
    let usb_device = usb_candidates.iter().find(|d| d.id == req.usb_device_id);
    let Some(usb_device) = usb_device else {
        return Err(AppError::not_found(
            "usb_device_not_found",
            "Requested USB device was not found among removable/USB devices.",
        ));
    };

    ensure_usb_candidate_suitable(usb_device)?;
    ensure_usb_real_breakglass(provisioning_mode)?;
    ensure_usb_real_allowlisted(provisioning_mode, &req.usb_device_id)?;

    let bootable_usb_root = data_path(&["bootable_usb"]);
    let _ = std::fs::create_dir_all(&bootable_usb_root);
    let output_path = bootable_usb_root
        .join(&req.session_id)
        .to_string_lossy()
        .into_owned();
    let _ = std::fs::create_dir_all(&output_path);
    let bundled_runtime = bundle_offline_runtime_artifacts(FsPath::new(&output_path))?;
    let (bootable_verified, provision_report_path) = run_usb_provisioning(
        provisioning_mode,
        &req.usb_device_id,
        &output_path,
    )?;

    let _ = std::fs::write(format!("{}/wipe_manifest.json", output_path), manifest_raw);

    let ingest_template = serde_json::json!({
        "schema_version": default_offline_result_schema_version(),
        "session_id": manifest.session_id,
        "target_device_id": manifest.target_device_id,
        "target_device_model": manifest.target_device_model,
        "target_device_size_gb": manifest.target_device_size_gb,
        "verification_passed": false,
        "verification_notes": "Populate after offline execution and verification review.",
        "completion_status": "inconclusive",
        "verification_evidence": {
            "sample_blocks_checked": 0,
            "sample_blocks_anomalies": 0,
            "checksum_algorithm": "sha256",
            "verification_tool": "offline_runtime",
            "operator_id": "operator-id"
        }
    });
    let _ = std::fs::write(
        format!("{}/offline_result_ingest_template.json", output_path),
        serde_json::to_string_pretty(&ingest_template).unwrap_or_else(|_| "{}".to_string()),
    );

    let runtime_note = if bundled_runtime {
        format!(
            "Bundled runtime: {} is included in this package.\n",
            offline_runtime_binary_name()
        )
    } else {
        format!(
            "Bundled runtime: {} was not found during package preparation. Build it first or set SECUREWIPE_OFFLINE_RUNTIME_BINARY before preparing USB. See OFFLINE_RUNTIME_STATUS.txt for details.\n",
            offline_runtime_binary_name()
        )
    };
    let instructions = format!(
        "SecureWipe Offline Boot Instructions\n\n1. Restart laptop\n2. Open boot menu (F12/Esc)\n3. Select USB device\n4. Run the offline runtime using wipe_manifest.json\n5. Confirm wipe by typing ERASE\n6. Review offline_result_ingest_template.json and replace it with the generated offline_result_ingest.json\n7. Return the result file to the in-app backend for ingest\n\nPrepared USB target id: {}\nSession: {}\n{}Offline runtime example: offline_runtime --manifest wipe_manifest.json --confirmation-text ERASE --output-dir .\n",
        req.usb_device_id, req.session_id, runtime_note
    );
    let _ = std::fs::write(format!("{}/README_OFFLINE.txt", output_path), instructions);

    let run_cmd = "@echo off\r\nsetlocal\r\nif not exist offline_runtime.exe (\r\n  echo offline_runtime.exe was not found in this folder.\r\n  echo Build it with cargo build -p securewipe-cli --bin offline_runtime or inspect OFFLINE_RUNTIME_STATUS.txt\r\n  exit /b 1\r\n)\r\noffline_runtime.exe --manifest wipe_manifest.json --confirmation-text ERASE --output-dir .\r\n";
    let _ = std::fs::write(format!("{}/RUN_OFFLINE_WIPE.cmd", output_path), run_cmd);

    let run_sh = "#!/usr/bin/env sh\nset -eu\nif [ ! -f ./offline_runtime ]; then\n  echo \"offline_runtime was not found in this folder.\"\n  echo \"Build it with cargo build -p securewipe-cli --bin offline_runtime or inspect OFFLINE_RUNTIME_STATUS.txt\"\n  exit 1\nfi\n./offline_runtime --manifest wipe_manifest.json --confirmation-text ERASE --output-dir .\n";
    let _ = std::fs::write(format!("{}/run_offline_wipe.sh", output_path), run_sh);

    manifest.phase = WipeSessionPhase::UsbPrepared;
    manifest.progress_percent = manifest.progress_percent.max(25);
    manifest.resume_required = true;
    manifest.resume_hint = Some("boot_into_offline_environment".to_string());
    write_session_manifest(&manifest);

    let operation_id = new_operation_id();
    log_operation_event(
        "usb_prepared",
        Some(&req.session_id),
        Some(&operation_id),
        &format!(
            "mode={} usb_device_id={} bootable_verified={}",
            provisioning_mode.as_str(),
            req.usb_device_id,
            bootable_verified
        ),
    );

    append_history(HistoryEntry {
        device_id: req.usb_device_id,
        model: "usb_builder".to_string(),
        recommendation: "offline_handoff".to_string(),
        explanation: format!(
            "Bootable USB handoff package prepared (mode: {}, bootable_verified: {}).",
            provisioning_mode.as_str(),
            bootable_verified
        ),
        timestamp: Utc::now().to_rfc3339(),
        operation_id: Some(operation_id),
        wipe_id: Some(req.session_id.clone()),
        phase: Some("usb_prepared".to_string()),
    });

    Ok(Json(PrepareUsbResponse {
        status: if provisioning_mode == UsbProvisionMode::Real {
            "bootable_usb_prepared_real".to_string()
        } else {
            "bootable_usb_prepared_simulation".to_string()
        },
        output_path,
        next_step: "Restart and boot from USB to run offline wipe environment.".to_string(),
        provisioning_mode: provisioning_mode.as_str().to_string(),
        provision_report_path,
        bootable_verified,
        phase: WipeSessionPhase::UsbPrepared,
        progress_percent: manifest.progress_percent,
        resume_required: manifest.resume_required,
        resume_hint: manifest.resume_hint,
    }))
}

#[cfg(test)]
mod tests {
    use axum::Json;
    use std::fs;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use super::{
        bundle_offline_runtime_artifacts,
        allow_unknown_detection_confidence, device_detection_confidence_sufficient,
        device_is_protected_target, device_meets_strict_targeting, ensure_usb_candidate_suitable,
        ensure_usb_real_breakglass,
        ensure_usb_real_allowlisted,
        execute_offline_wipe, ingest_offline_result, run_usb_provisioning,
        usb_overwrite_confirmation_valid, UsbProvisionMode,
    };
    use crate::api::types::{
        default_offline_result_schema_version, default_wipe_manifest_schema_version,
        OfflineCompletionStatus, OfflineExecuteRequest, OfflineResultIngestRequest,
        WipeSessionPhase,
    };
    use crate::api::storage::write_session_manifest;
    use crate::api::types::WipeSessionManifest;
    use crate::devices::{DetectionConfidenceLevel, Device, DeviceDetectionConfidence};

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[tokio::test]
    async fn execute_offline_wipe_rejects_wrong_confirmation() {
        let req = OfflineExecuteRequest {
            session_id: "session-x".to_string(),
            confirmation_text: "NO".to_string(),
        };

        let err = execute_offline_wipe(Json(req)).await.err().expect("expected error");
        assert_eq!(err.code, "offline_confirmation_invalid");
        assert_eq!(err.status.as_u16(), 422);
    }

    #[tokio::test]
    async fn ingest_offline_result_rejects_empty_session_id() {
        let req = OfflineResultIngestRequest {
            schema_version: default_offline_result_schema_version(),
            session_id: "".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: true,
            verification_notes: None,
            completion_status: OfflineCompletionStatus::Verified,
            verification_evidence: Some(crate::api::types::OfflineVerificationEvidence {
                sample_blocks_checked: 8,
                sample_blocks_anomalies: 0,
                checksum_algorithm: Some("sha256".to_string()),
                verification_tool: Some("offline-runtime-test".to_string()),
                operator_id: Some("tester".to_string()),
            }),
        };

        let err = ingest_offline_result(Json(req)).await.err().expect("expected error");
        assert_eq!(err.code, "session_id_required");
        assert_eq!(err.status.as_u16(), 422);
    }

    #[tokio::test]
    async fn ingest_offline_result_rejects_unsupported_schema_version() {
        let req = OfflineResultIngestRequest {
            schema_version: 999,
            session_id: "session-x".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: true,
            verification_notes: None,
            completion_status: OfflineCompletionStatus::Verified,
            verification_evidence: Some(crate::api::types::OfflineVerificationEvidence {
                sample_blocks_checked: 8,
                sample_blocks_anomalies: 0,
                checksum_algorithm: Some("sha256".to_string()),
                verification_tool: Some("offline-runtime-test".to_string()),
                operator_id: Some("tester".to_string()),
            }),
        };

        let err = ingest_offline_result(Json(req)).await.err().expect("expected error");
        assert_eq!(err.code, "unsupported_offline_result_schema_version");
        assert_eq!(err.status.as_u16(), 409);
    }

    #[tokio::test]
    async fn ingest_offline_result_rejects_invalid_session_phase() {
        let session_id = "phase-check-session".to_string();
        let manifest = WipeSessionManifest {
            schema_version: default_wipe_manifest_schema_version(),
            session_id: session_id.clone(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            mode: "offline".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            target_device_serial: None,
            target_detection_snapshot: None,
            target_detection_snapshot_sha256: None,
            target_detection_snapshot_signature: None,
            method: "overwrite".to_string(),
            estimated_minutes: 1,
            risk_level: "low".to_string(),
            final_confirmation_required: "ERASE".to_string(),
            phase: WipeSessionPhase::InAppPrepared,
            progress_percent: 10,
            resume_required: false,
            resume_hint: None,
        };
        write_session_manifest(&manifest);

        let req = OfflineResultIngestRequest {
            schema_version: default_offline_result_schema_version(),
            session_id: session_id.clone(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: true,
            verification_notes: None,
            completion_status: OfflineCompletionStatus::Verified,
            verification_evidence: Some(crate::api::types::OfflineVerificationEvidence {
                sample_blocks_checked: 8,
                sample_blocks_anomalies: 0,
                checksum_algorithm: Some("sha256".to_string()),
                verification_tool: Some("offline-runtime-test".to_string()),
                operator_id: Some("tester".to_string()),
            }),
        };

        let err = ingest_offline_result(Json(req)).await.err().expect("expected error");
        assert_eq!(err.code, "invalid_session_phase_for_result_ingest");
        assert_eq!(err.status.as_u16(), 409);

        let _ = fs::remove_file(format!("data/wipe_sessions/{}.json", session_id));
    }

    #[tokio::test]
    async fn ingest_offline_result_rejects_inconsistent_verified_status() {
        let req = OfflineResultIngestRequest {
            schema_version: default_offline_result_schema_version(),
            session_id: "session-x".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: true,
            verification_notes: None,
            completion_status: OfflineCompletionStatus::Partial,
            verification_evidence: None,
        };

        let err = ingest_offline_result(Json(req)).await.err().expect("expected error");
        assert_eq!(err.code, "offline_result_status_inconsistent");
        assert_eq!(err.status.as_u16(), 409);
    }

    #[tokio::test]
    async fn ingest_offline_result_requires_notes_for_review_status() {
        let req = OfflineResultIngestRequest {
            schema_version: default_offline_result_schema_version(),
            session_id: "session-x".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: false,
            verification_notes: None,
            completion_status: OfflineCompletionStatus::Failed,
            verification_evidence: None,
        };

        let err = ingest_offline_result(Json(req)).await.err().expect("expected error");
        assert_eq!(err.code, "verification_notes_required");
        assert_eq!(err.status.as_u16(), 422);
    }

    #[test]
    fn protected_target_predicate_blocks_system_disks() {
        let protected = Device {
            id: "disk-system".to_string(),
            dev_type: "SSD".to_string(),
            model: "SystemDisk".to_string(),
            serial: Some("SER-SYS".to_string()),
            size_gb: 512,
            allocated_gb: Some(200),
            partitions: vec![],
            connection: Some("SATA".to_string()),
            removable: Some(false),
            is_system: Some(true),
            smart_status: Some("OK".to_string()),
            temperature_c: Some(35.0),
            encrypted: false,
            hpa_dco: false,
            firmware: Some("FW1".to_string()),
            error: None,
            metadata: HashMap::new(),
            detection_confidence: DeviceDetectionConfidence::default(),
        };
        let non_protected = Device {
            is_system: Some(false),
            ..protected.clone()
        };

        assert!(device_is_protected_target(&protected));
        assert!(!device_is_protected_target(&non_protected));
    }

    #[test]
    fn strict_targeting_predicate_allows_removable_or_allowlisted() {
        let base = Device {
            id: "disk-data".to_string(),
            dev_type: "SSD".to_string(),
            model: "DataDisk".to_string(),
            serial: Some("SER-DATA".to_string()),
            size_gb: 1024,
            allocated_gb: Some(100),
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
        };

        let removable = Device {
            removable: Some(true),
            ..base.clone()
        };
        let allowlist = vec!["disk-data".to_string()];
        let empty = Vec::<String>::new();

        assert!(device_meets_strict_targeting(&removable, &empty));
        assert!(device_meets_strict_targeting(&base, &allowlist));
        assert!(!device_meets_strict_targeting(&base, &empty));
    }

    #[test]
    fn detection_confidence_predicate_requires_known_critical_fields() {
        let known = DeviceDetectionConfidence {
            encrypted: DetectionConfidenceLevel::Measured,
            hpa_dco: DetectionConfidenceLevel::Inferred,
            is_system: DetectionConfidenceLevel::Measured,
        };
        let unknown = DeviceDetectionConfidence {
            encrypted: DetectionConfidenceLevel::Unknown,
            hpa_dco: DetectionConfidenceLevel::Inferred,
            is_system: DetectionConfidenceLevel::Measured,
        };

        let known_device = Device {
            detection_confidence: known,
            ..base_device_for_confidence()
        };
        let unknown_device = Device {
            detection_confidence: unknown,
            ..base_device_for_confidence()
        };

        assert!(device_detection_confidence_sufficient(&known_device));
        assert!(!device_detection_confidence_sufficient(&unknown_device));
    }

    #[test]
    fn unknown_detection_override_flag_respected() {
        unsafe {
            std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
            std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION");
        }
        assert!(!allow_unknown_detection_confidence());

        unsafe { std::env::set_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", "1"); }
        assert!(allow_unknown_detection_confidence());

        unsafe {
            std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
            std::env::set_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION", "true");
        }
        assert!(allow_unknown_detection_confidence());

        unsafe {
            std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
            std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION");
        }
    }

    #[test]
    fn usb_overwrite_confirmation_token_validation_works() {
        assert!(usb_overwrite_confirmation_valid(Some("ERASE_USB")));
        assert!(usb_overwrite_confirmation_valid(Some("erase_usb")));
        assert!(!usb_overwrite_confirmation_valid(Some("ERASE")));
        assert!(!usb_overwrite_confirmation_valid(None));
    }

    #[test]
    fn usb_candidate_predicate_rejects_non_removable_or_too_small_devices() {
        unsafe { std::env::set_var("SECUREWIPE_USB_MIN_SIZE_GB", "8"); }

        let non_removable = Device {
            removable: Some(false),
            dev_type: "SSD".to_string(),
            size_gb: 64,
            ..base_device_for_confidence()
        };
        let too_small = Device {
            removable: Some(true),
            dev_type: "USB".to_string(),
            size_gb: 4,
            ..base_device_for_confidence()
        };

        let err_non_removable = ensure_usb_candidate_suitable(&non_removable)
            .err()
            .expect("expected non-removable rejection");
        assert_eq!(err_non_removable.code, "usb_device_not_removable");

        let err_too_small = ensure_usb_candidate_suitable(&too_small)
            .err()
            .expect("expected too-small rejection");
        assert_eq!(err_too_small.code, "usb_device_size_insufficient");

        unsafe { std::env::remove_var("SECUREWIPE_USB_MIN_SIZE_GB"); }
    }

    #[test]
    fn usb_real_provisioning_requires_command() {
        let root = std::env::temp_dir().join(format!(
            "securewipe-usb-provision-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&root).expect("failed to create temp dir");

        unsafe {
            std::env::set_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED", "1");
            std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
            std::env::remove_var("SECUREWIPE_USB_PROVISION_COMMAND");
            std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON");
            std::env::remove_var("SECUREWIPE_USB_PROVISION_ARGS_JSON");
        }

        let err = run_usb_provisioning(
            UsbProvisionMode::Real,
            "usb-test",
            root.to_string_lossy().as_ref(),
        )
        .err()
        .expect("expected missing command error");
        assert_eq!(err.code, "usb_real_provision_command_missing");

        unsafe {
            std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED");
            std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
            std::env::remove_var("SECUREWIPE_USB_PROVISION_COMMAND");
            std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON");
            std::env::remove_var("SECUREWIPE_USB_PROVISION_ARGS_JSON");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn usb_real_provisioning_requires_explicit_allowlist() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

        unsafe {
            std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        }

        let missing_err = ensure_usb_real_allowlisted(UsbProvisionMode::Real, "usb-test")
            .err()
            .expect("expected missing allowlist error");
        assert_eq!(missing_err.code, "usb_real_allowlist_required");

        unsafe {
            std::env::set_var("SECUREWIPE_USB_REAL_ALLOWLIST", "usb-allowed");
        }

        let not_allowed_err = ensure_usb_real_allowlisted(UsbProvisionMode::Real, "usb-test")
            .err()
            .expect("expected not-allowlisted error");
        assert_eq!(
            not_allowed_err.code,
            "usb_device_not_allowlisted_for_real_provision"
        );

        let ok = ensure_usb_real_allowlisted(UsbProvisionMode::Real, "usb-allowed");
        assert!(ok.is_ok());

        unsafe {
            std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        }
    }

    #[test]
    fn usb_real_provisioning_requires_breakglass_flag() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

        unsafe {
            std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
        }

        let err = ensure_usb_real_breakglass(UsbProvisionMode::Real)
            .err()
            .expect("expected missing breakglass error");
        assert_eq!(err.code, "usb_real_breakglass_required");

        unsafe {
            std::env::set_var("SECUREWIPE_USB_REAL_BREAKGLASS", "1");
        }
        let ok = ensure_usb_real_breakglass(UsbProvisionMode::Real);
        assert!(ok.is_ok());

        unsafe {
            std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
        }
    }

    fn base_device_for_confidence() -> Device {
        Device {
            id: "disk-check".to_string(),
            dev_type: "SSD".to_string(),
            model: "CheckDisk".to_string(),
            serial: Some("SER-CHECK".to_string()),
            size_gb: 256,
            allocated_gb: Some(64),
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
    fn bundle_offline_runtime_artifacts_copies_configured_binary() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");
        let root = std::env::temp_dir().join(format!(
            "securewipe-bundle-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let output_dir = root.join("package");
        let binary_source = root.join(super::offline_runtime_binary_name());
        fs::create_dir_all(&output_dir).expect("failed to create output dir");
        fs::write(&binary_source, b"fake-binary").expect("failed to write fake binary");

        unsafe {
            std::env::set_var("SECUREWIPE_OFFLINE_RUNTIME_BINARY", &binary_source);
        }

        let bundled = bundle_offline_runtime_artifacts(&output_dir).expect("bundle should succeed");

        unsafe {
            std::env::remove_var("SECUREWIPE_OFFLINE_RUNTIME_BINARY");
        }

        assert!(bundled);
        assert!(output_dir.join(super::offline_runtime_binary_name()).is_file());
        let status = fs::read_to_string(output_dir.join("OFFLINE_RUNTIME_STATUS.txt"))
            .expect("status file should exist");
        assert!(status.contains("bundled"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bundle_offline_runtime_artifacts_reports_missing_binary() {
        let _env_guard = env_test_lock().lock().expect("env test lock poisoned");
        let root = std::env::temp_dir().join(format!(
            "securewipe-bundle-missing-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let output_dir = root.join("package");
        fs::create_dir_all(&output_dir).expect("failed to create output dir");

        unsafe {
            std::env::set_var("SECUREWIPE_OFFLINE_RUNTIME_BINARY", root.join("missing-binary"));
            std::env::set_var("SECUREWIPE_DISABLE_OFFLINE_RUNTIME_AUTO_DISCOVERY", "1");
        }

        let bundled = bundle_offline_runtime_artifacts(&output_dir).expect("bundle should not error when missing");

        unsafe {
            std::env::remove_var("SECUREWIPE_OFFLINE_RUNTIME_BINARY");
            std::env::remove_var("SECUREWIPE_DISABLE_OFFLINE_RUNTIME_AUTO_DISCOVERY");
        }

        assert!(!bundled);
        let status = fs::read_to_string(output_dir.join("OFFLINE_RUNTIME_STATUS.txt"))
            .expect("status file should exist");
        assert!(status.contains("missing"));

        let _ = fs::remove_dir_all(&root);
    }
}
