use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;

use securewipe_core::api::certificate_crypto::{payload_sha256, verify_certificate_payload};
use securewipe_core::api::result_policy::validate_offline_result_contract;
use securewipe_core::api::types::{
    default_offline_result_schema_version, default_wipe_manifest_schema_version,
    OfflineCompletionStatus, OfflineResultIngestRequest, OfflineVerificationEvidence,
    WipeSessionManifest, WipeSessionPhase,
};
use securewipe_core::devices::{detect_devices, DetectionConfidenceLevel, Device};
use securewipe_core::engine::wipe::{perform_wipe_offline, WipeMode};

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CompletionStatusArg {
    Verified,
    Failed,
    Partial,
    Inconclusive,
}

impl From<CompletionStatusArg> for OfflineCompletionStatus {
    fn from(value: CompletionStatusArg) -> Self {
        match value {
            CompletionStatusArg::Verified => OfflineCompletionStatus::Verified,
            CompletionStatusArg::Failed => OfflineCompletionStatus::Failed,
            CompletionStatusArg::Partial => OfflineCompletionStatus::Partial,
            CompletionStatusArg::Inconclusive => OfflineCompletionStatus::Inconclusive,
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Execute SecureWipe offline manifest and emit ingest-ready result artifacts")]
struct Args {
    #[arg(long)]
    manifest: PathBuf,

    #[arg(long)]
    confirmation_text: String,

    #[arg(long, default_value = ".")]
    output_dir: PathBuf,

    #[arg(long, value_enum, default_value_t = CompletionStatusArg::Inconclusive)]
    completion_status: CompletionStatusArg,

    #[arg(long)]
    verification_passed: bool,

    #[arg(long)]
    verification_notes: Option<String>,

    #[arg(long)]
    verification_sample_blocks: Option<u32>,

    #[arg(long, default_value_t = 0)]
    verification_sample_anomalies: u32,

    #[arg(long)]
    verification_checksum_algorithm: Option<String>,

    #[arg(long)]
    verification_tool: Option<String>,

    #[arg(long)]
    operator_id: Option<String>,

    #[arg(long, default_value_t = false)]
    print_json: bool,
}

#[derive(Serialize)]
struct OfflineExecutionReport {
    session_id: String,
    executed_at: String,
    manifest_path: String,
    target_device_id: String,
    target_device_model: String,
    target_device_size_gb: u64,
    runtime_snapshot_sha256: String,
    stage1_snapshot_sha256: Option<String>,
    stage1_signature_present: bool,
    execution_status: String,
    wipe_mode: String,
    wipe_message: String,
    offline_result_ingest_path: String,
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}

fn strict_targeting_enabled() -> bool {
    std::env::var("SECUREWIPE_STRICT_TARGETING")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true)
}

fn allow_unknown_detection_confidence() -> bool {
    parse_env_bool("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", false)
        || parse_env_bool("SECUREWIPE_ALLOW_UNKNOWN_DETECTION", false)
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

fn device_meets_strict_targeting(device: &Device, allowlist: &[String]) -> bool {
    if device.removable.unwrap_or(false) {
        return true;
    }

    allowlist.iter().any(|id| id == &device.id)
}

fn device_detection_confidence_sufficient(device: &Device) -> bool {
    device.detection_confidence.is_system != DetectionConfidenceLevel::Unknown
        && device.detection_confidence.encrypted != DetectionConfidenceLevel::Unknown
        && device.detection_confidence.hpa_dco != DetectionConfidenceLevel::Unknown
}

fn manifest_matches_device(manifest: &WipeSessionManifest, device: &Device) -> bool {
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

fn build_detection_snapshot(device: &Device) -> serde_json::Value {
    let partitions = device
        .partitions
        .iter()
        .map(|partition| {
            json!({
                "name": partition.name,
                "mount_point": partition.mount_point,
                "size_gb": partition.size_gb,
                "used_gb": partition.used_gb,
                "fs_type": partition.fs_type,
                "is_system": partition.is_system,
                "is_boot": partition.is_boot,
                "encrypted": partition.encrypted
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

fn read_manifest(path: &Path) -> Result<WipeSessionManifest> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest: {}", path.display()))?;
    let manifest = serde_json::from_str::<WipeSessionManifest>(&raw)
        .with_context(|| format!("failed to parse manifest json: {}", path.display()))?;

    if manifest.schema_version != default_wipe_manifest_schema_version() {
        return Err(anyhow!(
            "unsupported manifest schema version {} (expected {})",
            manifest.schema_version,
            default_wipe_manifest_schema_version()
        ));
    }

    Ok(manifest)
}

fn validate_manifest_runtime(manifest: &WipeSessionManifest, confirmation_text: &str) -> Result<()> {
    if confirmation_text.trim().to_uppercase() != manifest.final_confirmation_required.trim().to_uppercase() {
        return Err(anyhow!(
            "confirmation text mismatch: expected {}",
            manifest.final_confirmation_required
        ));
    }

    if !manifest.phase.can_transition_to(&WipeSessionPhase::OfflineStarted) {
        return Err(anyhow!(
            "manifest phase {} is not valid for offline execution start",
            manifest.phase
        ));
    }

    Ok(())
}

fn validate_runtime_device(manifest: &WipeSessionManifest, device: &Device) -> Result<String> {
    if device.is_system.unwrap_or(false) {
        return Err(anyhow!("target device is marked as a protected system disk"));
    }

    if strict_targeting_enabled() {
        let allowlist = target_allowlist_ids();
        if !device_meets_strict_targeting(device, &allowlist) {
            return Err(anyhow!(
                "strict targeting allows only removable devices or allowlisted IDs"
            ));
        }
    }

    if !device_detection_confidence_sufficient(device) && !allow_unknown_detection_confidence() {
        return Err(anyhow!(
            "critical detection confidence is unknown; set SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE=1 only for controlled emergency workflows"
        ));
    }

    if !manifest_matches_device(manifest, device) {
        return Err(anyhow!("runtime device identity does not match manifest target identity"));
    }

    if manifest.target_detection_snapshot.is_some()
        || manifest.target_detection_snapshot_sha256.is_some()
        || manifest.target_detection_snapshot_signature.is_some()
    {
        let stage1_snapshot = manifest
            .target_detection_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("manifest is missing target_detection_snapshot"))?;
        let stage1_snapshot_sha256 = manifest
            .target_detection_snapshot_sha256
            .as_ref()
            .ok_or_else(|| anyhow!("manifest is missing target_detection_snapshot_sha256"))?;

        let recomputed_stage1_sha = payload_sha256(stage1_snapshot)
            .map_err(|error| anyhow!("failed to hash stage-1 snapshot: {}", error.message))?;
        if recomputed_stage1_sha != *stage1_snapshot_sha256 {
            return Err(anyhow!("manifest stage-1 snapshot hash does not match payload"));
        }

        if let Some(signature) = manifest.target_detection_snapshot_signature.as_ref() {
            let verified = verify_certificate_payload(
                stage1_snapshot,
                &signature.signature_base64,
                &signature.public_key_base64,
            )
            .map_err(|error| anyhow!("failed to verify stage-1 snapshot signature: {}", error.message))?;
            if !verified {
                return Err(anyhow!("manifest stage-1 snapshot signature is invalid"));
            }
            if signature.payload_sha256 != *stage1_snapshot_sha256 {
                return Err(anyhow!("manifest stage-1 snapshot signature hash does not match manifest hash"));
            }
        }

        let stage2_snapshot = build_detection_snapshot(device);
        let stage2_snapshot_sha256 = payload_sha256(&stage2_snapshot)
            .map_err(|error| anyhow!("failed to hash stage-2 snapshot: {}", error.message))?;
        if stage2_snapshot_sha256 != *stage1_snapshot_sha256 {
            return Err(anyhow!(
                "stage-2 runtime detection snapshot does not match the recorded stage-1 snapshot"
            ));
        }

        return Ok(stage2_snapshot_sha256);
    }

    let stage2_snapshot = build_detection_snapshot(device);
    payload_sha256(&stage2_snapshot)
        .map_err(|error| anyhow!("failed to hash runtime detection snapshot: {}", error.message))
}

fn default_notes_for_status(status: &OfflineCompletionStatus) -> Option<String> {
    if status.is_verified() {
        None
    } else {
        Some("Offline execution completed, but verification review is still required.".to_string())
    }
}

fn build_ingest_request(args: &Args, manifest: &WipeSessionManifest) -> Result<OfflineResultIngestRequest> {
    let completion_status: OfflineCompletionStatus = args.completion_status.into();
    let verification_passed = if completion_status.is_verified() {
        true
    } else {
        args.verification_passed
    };

    let verification_notes = args
        .verification_notes
        .clone()
        .or_else(|| default_notes_for_status(&completion_status));

    let verification_evidence = if completion_status.is_verified() {
        Some(OfflineVerificationEvidence {
            sample_blocks_checked: args.verification_sample_blocks.unwrap_or(8),
            sample_blocks_anomalies: args.verification_sample_anomalies,
            checksum_algorithm: Some(
                args.verification_checksum_algorithm
                    .clone()
                    .unwrap_or_else(|| "sha256".to_string()),
            ),
            verification_tool: Some(
                args.verification_tool
                    .clone()
                    .unwrap_or_else(|| "securewipe_offline_runtime".to_string()),
            ),
            operator_id: args.operator_id.clone(),
        })
    } else {
        args.verification_sample_blocks.map(|sample_blocks_checked| OfflineVerificationEvidence {
            sample_blocks_checked,
            sample_blocks_anomalies: args.verification_sample_anomalies,
            checksum_algorithm: args.verification_checksum_algorithm.clone(),
            verification_tool: args.verification_tool.clone(),
            operator_id: args.operator_id.clone(),
        })
    };

    let request = OfflineResultIngestRequest {
        schema_version: default_offline_result_schema_version(),
        session_id: manifest.session_id.clone(),
        target_device_id: manifest.target_device_id.clone(),
        target_device_model: manifest.target_device_model.clone(),
        target_device_size_gb: manifest.target_device_size_gb,
        verification_passed,
        verification_notes,
        completion_status,
        verification_evidence,
    };

    validate_offline_result_contract(&request)
        .map_err(|error| anyhow!("offline result payload validation failed: {}", error.message))?;
    Ok(request)
}

fn write_bundle(
    output_dir: &Path,
    ingest_request: &OfflineResultIngestRequest,
    report: &OfflineExecutionReport,
) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory: {}", output_dir.display()))?;

    let ingest_path = output_dir.join("offline_result_ingest.json");
    fs::write(
        &ingest_path,
        serde_json::to_string_pretty(ingest_request).context("failed to serialize ingest request")?,
    )
    .with_context(|| format!("failed to write {}", ingest_path.display()))?;

    let report_path = output_dir.join("offline_execution_report.json");
    fs::write(
        &report_path,
        serde_json::to_string_pretty(report).context("failed to serialize execution report")?,
    )
    .with_context(|| format!("failed to write {}", report_path.display()))?;

    Ok((ingest_path, report_path))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let manifest = read_manifest(&args.manifest)?;
    validate_manifest_runtime(&manifest, &args.confirmation_text)?;

    let devices = detect_devices();
    let device = devices
        .iter()
        .find(|device| device.id == manifest.target_device_id)
        .ok_or_else(|| anyhow!("target device from manifest not found in runtime scan"))?;

    let runtime_snapshot_sha256 = validate_runtime_device(&manifest, device)?;
    let wipe_result = perform_wipe_offline(device)
        .map_err(|error| anyhow!("offline wipe execution failed: {}", error))?;

    let ingest_request = build_ingest_request(&args, &manifest)?;
    let report = OfflineExecutionReport {
        session_id: manifest.session_id.clone(),
        executed_at: Utc::now().to_rfc3339(),
        manifest_path: args.manifest.display().to_string(),
        target_device_id: manifest.target_device_id.clone(),
        target_device_model: manifest.target_device_model.clone(),
        target_device_size_gb: manifest.target_device_size_gb,
        runtime_snapshot_sha256,
        stage1_snapshot_sha256: manifest.target_detection_snapshot_sha256.clone(),
        stage1_signature_present: manifest.target_detection_snapshot_signature.is_some(),
        execution_status: "completed".to_string(),
        wipe_mode: match wipe_result.mode {
            WipeMode::Simulation => "simulation".to_string(),
            WipeMode::Destructive => "destructive".to_string(),
        },
        wipe_message: wipe_result.message,
        offline_result_ingest_path: args.output_dir.join("offline_result_ingest.json").display().to_string(),
    };

    let (ingest_path, report_path) = write_bundle(&args.output_dir, &ingest_request, &report)?;

    if args.print_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "offline_runtime_completed",
                "session_id": manifest.session_id,
                "ingest_result_path": ingest_path,
                "execution_report_path": report_path
            }))?
        );
    } else {
        println!("Offline runtime completed for session {}", manifest.session_id);
        println!("Ingest-ready result: {}", ingest_path.display());
        println!("Execution report: {}", report_path.display());
    }

    Ok(())
}
