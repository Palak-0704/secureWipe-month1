use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WipeManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub session_id: String,
    pub created_at: String,
    pub mode: String,
    pub target_device_id: String,
    pub target_device_model: String,
    pub target_device_size_gb: u64,
    #[serde(default)]
    pub target_device_serial: Option<String>,
    pub method: String,
    pub estimated_minutes: u32,
    pub risk_level: String,
    pub final_confirmation_required: String,
    pub phase: String,
    #[serde(default)]
    pub progress_percent: u8,
    // Stage-1 detection snapshot fields used for parity check
    #[serde(default)]
    pub target_detection_snapshot: Option<serde_json::Value>,
    #[serde(default)]
    pub target_detection_snapshot_sha256: Option<String>,
}

fn default_schema_version() -> u32 {
    1
}
