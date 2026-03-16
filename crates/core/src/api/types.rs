use serde::{Deserialize, Serialize};
use std::fmt;

pub const CURRENT_WIPE_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const CURRENT_OFFLINE_RESULT_SCHEMA_VERSION: u32 = 1;

pub fn default_wipe_manifest_schema_version() -> u32 {
    CURRENT_WIPE_MANIFEST_SCHEMA_VERSION
}

pub fn default_offline_result_schema_version() -> u32 {
    CURRENT_OFFLINE_RESULT_SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub device_id: String,
    pub model: String,
    pub recommendation: String,
    pub explanation: String,
    pub timestamp: String,
    #[serde(default)]
    pub operation_id: Option<String>,
    pub wipe_id: Option<String>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WipeRequest {
    pub wipe_id: String,
    pub confirmation_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LegacyWipeRequest {
    pub device_ids: Vec<String>,
    pub method: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StartWipeRequest {
    Confirmed(WipeRequest),
    Legacy(LegacyWipeRequest),
}

#[derive(Serialize)]
pub struct WipeStartResponse {
    pub status: String,
    pub wipe_id: String,
}

#[derive(Deserialize)]
pub struct OfflineExecuteRequest {
    pub session_id: String,
    pub confirmation_text: String,
}

#[derive(Serialize)]
pub struct OfflineExecuteResponse {
    pub status: String,
    pub session_id: String,
    pub phase: WipeSessionPhase,
    pub progress_percent: u8,
    pub resume_required: bool,
    pub resume_hint: Option<String>,
    pub mode: String,
    pub message: String,
}

#[derive(Deserialize, Serialize)]
pub struct OfflineResultIngestRequest {
    #[serde(default = "default_offline_result_schema_version")]
    pub schema_version: u32,
    pub session_id: String,
    pub target_device_id: String,
    pub target_device_model: String,
    pub target_device_size_gb: u64,
    pub verification_passed: bool,
    pub verification_notes: Option<String>,
    pub completion_status: OfflineCompletionStatus,
    #[serde(default)]
    pub verification_evidence: Option<OfflineVerificationEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineResultRecord {
    #[serde(default = "default_offline_result_schema_version")]
    pub schema_version: u32,
    pub session_id: String,
    pub target_device_id: String,
    pub target_device_model: String,
    pub target_device_size_gb: u64,
    pub verification_passed: bool,
    pub verification_notes: Option<String>,
    pub completion_status: OfflineCompletionStatus,
    #[serde(default)]
    pub verification_evidence: Option<OfflineVerificationEvidence>,
    pub ingested_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineVerificationEvidence {
    pub sample_blocks_checked: u32,
    pub sample_blocks_anomalies: u32,
    pub checksum_algorithm: Option<String>,
    pub verification_tool: Option<String>,
    pub operator_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfflineCompletionStatus {
    #[serde(alias = "ok", alias = "completed")]
    Verified,
    Failed,
    Partial,
    Inconclusive,
}

impl OfflineCompletionStatus {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }

    pub fn requires_review(&self) -> bool {
        matches!(self, Self::Partial | Self::Inconclusive)
    }
}

impl fmt::Display for OfflineCompletionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::Partial => "partial",
            Self::Inconclusive => "inconclusive",
        };
        write!(f, "{}", label)
    }
}

#[derive(Debug, Clone)]
pub struct CompletionArtifacts {
    pub manifest: WipeSessionManifest,
    pub result: OfflineResultRecord,
}

#[derive(Serialize)]
pub struct OfflineResultIngestResponse {
    pub status: String,
    pub session_id: String,
    pub phase: WipeSessionPhase,
    pub progress_percent: u8,
    pub resume_required: bool,
    pub resume_hint: Option<String>,
    pub reconciled: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipeConfirmationState {
    pub wipe_id: String,
    pub confirmation_token: String,
    pub device_ids: Vec<String>,
    pub target_identities: Vec<TargetIdentity>,
    pub method: String,
    pub created_at: String,
    pub flow_state: WipeFlowState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetIdentity {
    pub id: String,
    pub model: String,
    pub size_gb: u64,
    pub serial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WipeFlowState {
    Initialized,
    RiskAcknowledged,
    FinalConfirmed,
    StartedSimulation,
    CompletedSimulation,
}

#[derive(Deserialize)]
pub struct WipeConfirmInitRequest {
    pub device_ids: Vec<String>,
    pub method: String,
    pub expert_mode: Option<bool>,
}

#[derive(Serialize)]
pub struct WipeConfirmInitResponse {
    pub status: String,
    pub wipe_id: String,
    pub confirmation_token: String,
}

#[derive(Deserialize)]
pub struct WipeConfirmRiskRequest {
    pub wipe_id: String,
    pub confirmation_token: String,
    pub acknowledged: bool,
}

#[derive(Serialize)]
pub struct WipeConfirmRiskResponse {
    pub status: String,
}

#[derive(Deserialize)]
pub struct WipeConfirmFinalRequest {
    pub wipe_id: String,
    pub confirmation_token: String,
    pub confirmation_text: String,
}

#[derive(Serialize)]
pub struct WipeConfirmFinalResponse {
    pub status: String,
}

#[derive(Serialize)]
pub struct WipeProgressResponse {
    pub progress: u8,
    pub status: String,
}

#[derive(Deserialize)]
pub struct AdvisorRequest {
    pub device_ids: Vec<String>,
    pub compliance: String,
}

#[derive(Serialize)]
pub struct AdvisorResponse {
    pub method: String,
    pub estimated_minutes: u32,
    pub risk_level: String,
    pub explanation: String,
    pub confidence: f32,
    pub compliance_notes: Option<String>,
}

#[derive(Deserialize)]
pub struct ChatbotRequest {
    pub message: String,
    pub concise: Option<bool>,
}

#[derive(Serialize)]
pub struct ChatbotResponse {
    pub reply: String,
}

#[derive(Serialize)]
pub struct LogsResponse {
    pub logs: Vec<String>,
}

#[derive(Serialize)]
pub struct SystemHealthResponse {
    pub health: String,
    pub update_available: bool,
}

#[derive(Serialize)]
pub struct SecurityStatusResponse {
    pub status: String,
    pub protections_active: bool,
}

#[derive(Serialize)]
pub struct MvpPreflightResponse {
    pub host_os: String,
    pub uefi_ready: bool,
    pub secure_boot_supported_in_mvp: bool,
    pub mvp_supported: bool,
    pub notes: Vec<String>,
}

#[derive(Serialize)]
pub struct UsbCandidateResponse {
    pub id: String,
    pub model: String,
    pub size_gb: u64,
    pub removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipeSessionManifest {
    #[serde(default = "default_wipe_manifest_schema_version")]
    pub schema_version: u32,
    pub session_id: String,
    pub created_at: String,
    pub mode: String,
    pub target_device_id: String,
    pub target_device_model: String,
    pub target_device_size_gb: u64,
    #[serde(default)]
    pub target_device_serial: Option<String>,
    #[serde(default)]
    pub target_detection_snapshot: Option<serde_json::Value>,
    #[serde(default)]
    pub target_detection_snapshot_sha256: Option<String>,
    #[serde(default)]
    pub target_detection_snapshot_signature: Option<CertificateSignature>,
    pub method: String,
    pub estimated_minutes: u32,
    pub risk_level: String,
    pub final_confirmation_required: String,
    pub phase: WipeSessionPhase,
    #[serde(default)]
    pub progress_percent: u8,
    #[serde(default)]
    pub resume_required: bool,
    #[serde(default)]
    pub resume_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WipeSessionPhase {
    InAppPrepared,
    UsbPrepared,
    RebootPending,
    OfflineStarted,
    Wiping,
    Verified,
    Certified,
    Completed,
    Failed,
}

impl WipeSessionPhase {
    pub fn can_transition_to(&self, next: &Self) -> bool {
        use WipeSessionPhase::*;

        if self == next {
            return true;
        }

        matches!((self, next),
            (InAppPrepared, UsbPrepared)
                | (UsbPrepared, RebootPending)
                | (UsbPrepared, OfflineStarted)
                | (RebootPending, OfflineStarted)
                | (OfflineStarted, Wiping)
                | (OfflineStarted, Failed)
                | (Wiping, Verified)
                | (Wiping, Failed)
                | (Verified, Certified)
                | (Verified, Completed)
                | (Certified, Completed)
        )
    }
}

impl fmt::Display for WipeSessionPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::InAppPrepared => "in_app_prepared",
            Self::UsbPrepared => "usb_prepared",
            Self::RebootPending => "reboot_pending",
            Self::OfflineStarted => "offline_started",
            Self::Wiping => "wiping",
            Self::Verified => "verified",
            Self::Certified => "certified",
            Self::Completed => "completed",
            Self::Failed => "failed",
        };
        write!(f, "{}", label)
    }
}

#[derive(Deserialize)]
pub struct CreateWipeSessionRequest {
    pub mode: String,
    pub target_device_id: String,
    pub compliance: Option<String>,
}

#[derive(Serialize)]
pub struct CreateWipeSessionResponse {
    pub status: String,
    pub session_id: String,
    pub manifest_path: String,
    pub phase: WipeSessionPhase,
    pub progress_percent: u8,
    pub resume_required: bool,
    pub resume_hint: Option<String>,
}

#[derive(Deserialize)]
pub struct PrepareUsbRequest {
    pub session_id: String,
    pub usb_device_id: String,
    #[serde(default)]
    pub usb_overwrite_confirmation_text: Option<String>,
}

#[derive(Serialize)]
pub struct PrepareUsbResponse {
    pub status: String,
    pub output_path: String,
    pub next_step: String,
    pub provisioning_mode: String,
    pub provision_report_path: String,
    pub bootable_verified: bool,
    pub phase: WipeSessionPhase,
    pub progress_percent: u8,
    pub resume_required: bool,
    pub resume_hint: Option<String>,
}

#[derive(Serialize)]
pub struct SessionStatusResponse {
    pub status: String,
    pub session_id: String,
    pub phase: WipeSessionPhase,
    pub progress_percent: u8,
    pub resume_required: bool,
    pub resume_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResumeAction {
    PrepareUsb,
    RebootToOffline,
    AwaitOfflineResultIngest,
    ReviewCompletion,
    ManualInterventionRequired,
}

#[derive(Serialize)]
pub struct ResumeSessionResponse {
    pub status: String,
    pub session_id: String,
    pub phase: WipeSessionPhase,
    pub progress_percent: u8,
    pub resume_required: bool,
    pub recommended_action: ResumeAction,
    pub resume_hint: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct SessionArtifactsResponse {
    pub status: String,
    pub session_id: String,
    pub manifest_phase: WipeSessionPhase,
    pub manifest_schema_version: u32,
    pub result_schema_version: u32,
    pub verification_passed: bool,
    pub completion_status: OfflineCompletionStatus,
    pub target_device_id: String,
    pub target_device_model: String,
    pub target_device_size_gb: u64,
    pub artifact_consistent: bool,
}

#[derive(Serialize)]
pub struct CertificateData {
    pub wipe_id: String,
    pub generated_at: String,
    pub mode: String,
    pub method: String,
    pub status: String,
    pub recovery_risk: String,
    pub devices: Vec<CertificateDevice>,
    pub log_count: usize,
}

#[derive(Serialize)]
pub struct CertificateDevice {
    pub id: String,
    pub model: String,
}

#[derive(Serialize)]
pub struct CertificateResponse {
    pub certificate: serde_json::Value,
    pub signature_sha256: String,
    pub signature: CertificateSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateSignature {
    pub algorithm: String,
    pub signer_source: String,
    pub public_key_base64: String,
    pub signature_base64: String,
    pub payload_sha256: String,
}

#[derive(Deserialize)]
pub struct CertificateVerifyRequest {
    pub certificate: serde_json::Value,
    pub public_key_base64: String,
    pub signature_base64: String,
}

#[derive(Serialize)]
pub struct CertificateVerifyResponse {
    pub status: String,
    pub algorithm: String,
    pub verified: bool,
    pub payload_sha256: String,
}

#[derive(Serialize)]
pub struct CertificateReviewResponse {
    pub status: String,
    pub wipe_id: String,
    pub manifest_phase: WipeSessionPhase,
    pub completion_status: OfflineCompletionStatus,
    pub verification_passed: bool,
    pub certificate_eligible: bool,
    pub signature_verified: bool,
    pub recommended_action: String,
    pub issues: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_evidence: Option<OfflineVerificationEvidence>,
}

#[cfg(test)]
mod tests {
    use super::WipeSessionPhase;

    #[test]
    fn wipe_session_phase_transition_matrix_is_deterministic() {
        use WipeSessionPhase::*;

        let phases = vec![
            InAppPrepared,
            UsbPrepared,
            RebootPending,
            OfflineStarted,
            Wiping,
            Verified,
            Certified,
            Completed,
            Failed,
        ];

        let allowed = [
            (InAppPrepared, InAppPrepared),
            (InAppPrepared, UsbPrepared),
            (UsbPrepared, UsbPrepared),
            (UsbPrepared, RebootPending),
            (UsbPrepared, OfflineStarted),
            (RebootPending, RebootPending),
            (RebootPending, OfflineStarted),
            (OfflineStarted, OfflineStarted),
            (OfflineStarted, Wiping),
            (OfflineStarted, Failed),
            (Wiping, Wiping),
            (Wiping, Verified),
            (Wiping, Failed),
            (Verified, Verified),
            (Verified, Certified),
            (Verified, Completed),
            (Certified, Certified),
            (Certified, Completed),
            (Completed, Completed),
            (Failed, Failed),
        ];

        for from in &phases {
            for to in &phases {
                let expected = allowed.iter().any(|(a, b)| a == from && b == to);
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected transition result: {:?} -> {:?}",
                    from,
                    to
                );
            }
        }
    }
}
