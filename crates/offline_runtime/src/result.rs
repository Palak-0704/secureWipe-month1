use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct OfflineVerificationEvidence {
    pub sample_blocks_checked: u32,
    pub sample_blocks_anomalies: u32,
    pub checksum_algorithm: Option<String>,
    pub verification_tool: Option<String>,
    pub operator_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OfflineResult {
    pub schema_version: u32,
    pub session_id: String,
    pub target_device_id: String,
    pub target_device_model: String,
    pub target_device_size_gb: u64,
    pub verification_passed: bool,
    pub verification_notes: Option<String>,
    pub completion_status: String,
    pub verification_evidence: Option<OfflineVerificationEvidence>,
    pub completed_at: String,
}

pub fn write_result(output_dir: &Path, result: &OfflineResult) -> anyhow::Result<()> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| anyhow::anyhow!("cannot create output dir: {}", e))?;
    let path = output_dir.join("offline_result_ingest.json");
    let json = serde_json::to_string_pretty(result)
        .map_err(|e| anyhow::anyhow!("serialisation failed: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| anyhow::anyhow!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}
