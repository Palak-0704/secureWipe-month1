use crate::devices::Device;
use std::path::PathBuf;

use super::types::{
    CompletionArtifacts, HistoryEntry, OfflineResultRecord, TargetIdentity,
    WipeConfirmationState, WipeSessionManifest,
};

pub fn now_id(prefix: &str) -> String {
    format!("{}-{}", prefix, chrono::Utc::now().timestamp_millis())
}

pub fn data_root() -> PathBuf {
    std::env::var("SECUREWIPE_DATA_DIR")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"))
}

pub fn data_path(parts: &[&str]) -> PathBuf {
    let mut path = data_root();
    for part in parts {
        path.push(part);
    }
    path
}

fn path_string(parts: &[&str]) -> String {
    data_path(parts).to_string_lossy().into_owned()
}

pub fn new_operation_id() -> String {
    now_id("op")
}

pub fn read_history() -> Vec<HistoryEntry> {
    let path = path_string(&["feedback_history.json"]);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<HistoryEntry>>(&s).ok())
        .unwrap_or_default()
}

pub fn write_history(entries: &[HistoryEntry]) {
    let _ = std::fs::create_dir_all(data_root());
    let _ = std::fs::write(
        data_path(&["feedback_history.json"]),
        serde_json::to_string_pretty(entries).unwrap_or_else(|_| "[]".to_string()),
    );
}

pub fn append_history(entry: HistoryEntry) {
    let mut history = read_history();
    history.push(entry);
    write_history(&history);
}

pub fn collect_logs_for_wipe_id(wipe_id: &str) -> Vec<HistoryEntry> {
    read_history()
        .into_iter()
        .filter(|e| e.wipe_id.as_deref() == Some(wipe_id))
        .collect()
}

fn confirmation_state_path(wipe_id: &str) -> String {
    path_string(&["confirmations", &format!("{}.json", wipe_id)])
}

pub fn read_confirmation_state(wipe_id: &str) -> Option<WipeConfirmationState> {
    let path = confirmation_state_path(wipe_id);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<WipeConfirmationState>(&s).ok())
}

pub fn write_confirmation_state(state: &WipeConfirmationState) {
    let _ = std::fs::create_dir_all(data_path(&["confirmations"]));
    let path = confirmation_state_path(&state.wipe_id);
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(state).unwrap_or_else(|_| "{}".to_string()),
    );
}

pub fn identity_matches_device(identity: &TargetIdentity, device: &Device) -> bool {
    if identity.id != device.id {
        return false;
    }
    if identity.model != device.model {
        return false;
    }
    if identity.size_gb != device.size_gb {
        return false;
    }
    identity.serial == device.serial
}

pub fn session_manifest_path(session_id: &str) -> String {
    path_string(&["wipe_sessions", &format!("{}.json", session_id)])
}

pub fn read_session_manifest(session_id: &str) -> Option<WipeSessionManifest> {
    let path = session_manifest_path(session_id);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<WipeSessionManifest>(&s).ok())
}

pub fn write_session_manifest(manifest: &WipeSessionManifest) {
    let _ = std::fs::create_dir_all(data_path(&["wipe_sessions"]));
    let path = session_manifest_path(&manifest.session_id);
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(manifest).unwrap_or_else(|_| "{}".to_string()),
    );
}

pub fn offline_result_path(session_id: &str) -> String {
    path_string(&["offline_results", &format!("{}.json", session_id)])
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn read_offline_result(session_id: &str) -> Option<OfflineResultRecord> {
    let path = offline_result_path(session_id);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<OfflineResultRecord>(&s).ok())
}

pub fn write_offline_result(record: &OfflineResultRecord) {
    let _ = std::fs::create_dir_all(data_path(&["offline_results"]));
    let path = offline_result_path(&record.session_id);
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(record).unwrap_or_else(|_| "{}".to_string()),
    );
}

pub fn read_completion_artifacts(session_id: &str) -> Option<CompletionArtifacts> {
    let manifest = read_session_manifest(session_id)?;
    let result = read_offline_result(session_id)?;
    Some(CompletionArtifacts { manifest, result })
}

pub fn list_sessions() -> Vec<WipeSessionManifest> {
    let dir = data_path(&["wipe_sessions"]);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut sessions: Vec<WipeSessionManifest> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            std::fs::read_to_string(entry.path())
                .ok()
                .and_then(|s| serde_json::from_str::<WipeSessionManifest>(&s).ok())
        })
        .collect();
    // Most-recent first
    sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    sessions
}

#[cfg(test)]
mod tests {
    use super::{offline_result_path, read_offline_result, write_offline_result};
    use crate::api::types::{
        default_offline_result_schema_version, OfflineCompletionStatus, OfflineResultRecord,
        OfflineVerificationEvidence,
    };

    #[test]
    fn offline_result_roundtrip_uses_typed_record() {
        let record = OfflineResultRecord {
            schema_version: default_offline_result_schema_version(),
            session_id: "storage-roundtrip-session".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: true,
            verification_notes: Some("all good".to_string()),
            completion_status: OfflineCompletionStatus::Verified,
            verification_evidence: Some(OfflineVerificationEvidence {
                sample_blocks_checked: 8,
                sample_blocks_anomalies: 0,
                checksum_algorithm: Some("sha256".to_string()),
                verification_tool: Some("storage-test".to_string()),
                operator_id: Some("tester".to_string()),
            }),
            ingested_at: "2026-01-01T00:00:00Z".to_string(),
        };

        write_offline_result(&record);
        let loaded = read_offline_result(&record.session_id).expect("expected stored result");
        assert_eq!(loaded.schema_version, record.schema_version);
        assert_eq!(loaded.session_id, record.session_id);
        assert_eq!(loaded.target_device_id, record.target_device_id);

        let _ = std::fs::remove_file(offline_result_path(&record.session_id));
    }
}
