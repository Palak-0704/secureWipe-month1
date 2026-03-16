use super::types::{OfflineCompletionStatus, OfflineVerificationEvidence};

fn parse_env_u32(name: &str, default: u32, min: u32, max: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn min_verified_sample_blocks() -> u32 {
    parse_env_u32("SECUREWIPE_ANOMALY_MIN_VERIFIED_SAMPLE_BLOCKS", 8, 1, 4096)
}

fn notes_contain_anomaly_keywords(notes: &str) -> bool {
    let lowered = notes.to_ascii_lowercase();
    ["anomaly", "error", "failed", "mismatch", "tamper", "corrupt"]
        .iter()
        .any(|token| lowered.contains(token))
}

pub fn detect_offline_result_anomalies(
    verification_passed: bool,
    completion_status: &OfflineCompletionStatus,
    verification_notes: Option<&str>,
    verification_evidence: Option<&OfflineVerificationEvidence>,
) -> Vec<String> {
    let mut anomalies = Vec::new();

    if verification_passed && !completion_status.is_verified() {
        anomalies.push(
            "Anomaly detector: verification_passed=true but completion_status is not verified."
                .to_string(),
        );
    }

    if !verification_passed && completion_status.is_verified() {
        anomalies.push(
            "Anomaly detector: completion_status=verified but verification_passed=false."
                .to_string(),
        );
    }

    if completion_status.is_verified() {
        if let Some(notes) = verification_notes {
            if !notes.trim().is_empty() && notes_contain_anomaly_keywords(notes) {
                anomalies.push(
                    "Anomaly detector: verification notes contain anomaly/error keywords for a verified result."
                        .to_string(),
                );
            }
        }

        if let Some(evidence) = verification_evidence {
            if evidence.sample_blocks_checked < min_verified_sample_blocks() {
                anomalies.push(format!(
                    "Anomaly detector: verified result sampled only {} blocks, below minimum threshold {}.",
                    evidence.sample_blocks_checked,
                    min_verified_sample_blocks()
                ));
            }

            if evidence.operator_id.as_deref().map(|v| v.trim().is_empty()).unwrap_or(true) {
                anomalies.push(
                    "Anomaly detector: verified result missing operator_id in verification evidence."
                        .to_string(),
                );
            }
        }
    }

    anomalies
}

#[cfg(test)]
mod tests {
    use super::detect_offline_result_anomalies;
    use crate::api::types::{OfflineCompletionStatus, OfflineVerificationEvidence};

    #[test]
    fn no_anomaly_for_clean_verified_result() {
        let evidence = OfflineVerificationEvidence {
            sample_blocks_checked: 8,
            sample_blocks_anomalies: 0,
            checksum_algorithm: Some("sha256".to_string()),
            verification_tool: Some("runtime".to_string()),
            operator_id: Some("op-1".to_string()),
        };

        let anomalies = detect_offline_result_anomalies(
            true,
            &OfflineCompletionStatus::Verified,
            None,
            Some(&evidence),
        );

        assert!(anomalies.is_empty());
    }

    #[test]
    fn anomaly_for_verified_notes_with_error_keywords() {
        let evidence = OfflineVerificationEvidence {
            sample_blocks_checked: 8,
            sample_blocks_anomalies: 0,
            checksum_algorithm: Some("sha256".to_string()),
            verification_tool: Some("runtime".to_string()),
            operator_id: Some("op-1".to_string()),
        };

        let anomalies = detect_offline_result_anomalies(
            true,
            &OfflineCompletionStatus::Verified,
            Some("anomaly observed during verification"),
            Some(&evidence),
        );

        assert!(!anomalies.is_empty());
    }
}
