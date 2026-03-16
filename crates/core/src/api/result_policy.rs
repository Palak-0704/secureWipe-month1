use super::errors::AppError;
use super::types::{
    OfflineCompletionStatus, OfflineResultIngestRequest, OfflineVerificationEvidence,
};

fn has_non_empty(value: Option<&String>) -> bool {
    value.map(|v| !v.trim().is_empty()).unwrap_or(false)
}

fn validate_verification_evidence(
    evidence: &OfflineVerificationEvidence,
) -> Result<(), AppError> {
    if evidence.sample_blocks_checked == 0 {
        return Err(AppError::unprocessable_entity(
            "verification_evidence_invalid",
            "verification_evidence.sample_blocks_checked must be greater than zero.",
        ));
    }

    if evidence.sample_blocks_anomalies > evidence.sample_blocks_checked {
        return Err(AppError::unprocessable_entity(
            "verification_evidence_invalid",
            "verification_evidence.sample_blocks_anomalies cannot exceed sample_blocks_checked.",
        ));
    }

    Ok(())
}

pub fn validate_offline_result_contract(req: &OfflineResultIngestRequest) -> Result<(), AppError> {
    let has_notes = req
        .verification_notes
        .as_ref()
        .map(|notes| !notes.trim().is_empty())
        .unwrap_or(false);

    if req.verification_passed && !req.completion_status.is_verified() {
        return Err(AppError::conflict(
            "offline_result_status_inconsistent",
            "Verified offline results must use completion_status 'verified'.",
        ));
    }

    if !req.verification_passed && req.completion_status.is_verified() {
        return Err(AppError::conflict(
            "offline_result_status_inconsistent",
            "Offline results that failed verification cannot use completion_status 'verified'.",
        ));
    }

    if req.verification_passed && req.completion_status.is_verified() {
        let evidence = req.verification_evidence.as_ref().ok_or_else(|| {
            AppError::unprocessable_entity(
                "verification_evidence_required",
                "Verified offline results must include verification_evidence.",
            )
        })?;

        validate_verification_evidence(evidence)?;

        if evidence.sample_blocks_anomalies != 0 {
            return Err(AppError::conflict(
                "verification_evidence_inconsistent",
                "Verified offline results cannot report sample block anomalies.",
            ));
        }

        if !has_non_empty(evidence.checksum_algorithm.as_ref()) {
            return Err(AppError::unprocessable_entity(
                "verification_evidence_invalid",
                "Verified offline results require verification_evidence.checksum_algorithm.",
            ));
        }

        if !has_non_empty(evidence.verification_tool.as_ref()) {
            return Err(AppError::unprocessable_entity(
                "verification_evidence_invalid",
                "Verified offline results require verification_evidence.verification_tool.",
            ));
        }
    }

    if (!req.verification_passed || req.completion_status.requires_review()) && !has_notes {
        return Err(AppError::unprocessable_entity(
            "verification_notes_required",
            "Verification notes are required when offline verification fails or needs manual review.",
        ));
    }

    Ok(())
}

pub fn completion_outcome_message(
    verification_passed: bool,
    completion_status: &OfflineCompletionStatus,
) -> String {
    match completion_status {
        OfflineCompletionStatus::Verified if verification_passed => {
            "Offline wipe result ingested and verification passed.".to_string()
        }
        OfflineCompletionStatus::Failed => {
            "Offline wipe result ingested and verification failed.".to_string()
        }
        OfflineCompletionStatus::Partial => {
            "Offline wipe result ingested but only partial completion evidence was provided.".to_string()
        }
        OfflineCompletionStatus::Inconclusive => {
            "Offline wipe result ingested but the verification outcome is inconclusive.".to_string()
        }
        _ => "Offline wipe result ingested but requires manual review.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_offline_result_contract;
    use crate::api::types::{
        default_offline_result_schema_version, OfflineCompletionStatus,
        OfflineResultIngestRequest, OfflineVerificationEvidence,
    };

    fn base_verified_request() -> OfflineResultIngestRequest {
        OfflineResultIngestRequest {
            schema_version: default_offline_result_schema_version(),
            session_id: "session-1".to_string(),
            target_device_id: "disk1".to_string(),
            target_device_model: "model1".to_string(),
            target_device_size_gb: 100,
            verification_passed: true,
            verification_notes: None,
            completion_status: OfflineCompletionStatus::Verified,
            verification_evidence: Some(OfflineVerificationEvidence {
                sample_blocks_checked: 8,
                sample_blocks_anomalies: 0,
                checksum_algorithm: Some("sha256".to_string()),
                verification_tool: Some("securewipe_offline_runtime".to_string()),
                operator_id: Some("operator-1".to_string()),
            }),
        }
    }

    #[test]
    fn verified_result_requires_verification_evidence() {
        let mut req = base_verified_request();
        req.verification_evidence = None;

        let err = validate_offline_result_contract(&req)
            .err()
            .expect("expected validation error");
        assert_eq!(err.code, "verification_evidence_required");
    }

    #[test]
    fn verified_result_rejects_anomalies() {
        let mut req = base_verified_request();
        if let Some(evidence) = req.verification_evidence.as_mut() {
            evidence.sample_blocks_anomalies = 1;
        }

        let err = validate_offline_result_contract(&req)
            .err()
            .expect("expected validation error");
        assert_eq!(err.code, "verification_evidence_inconsistent");
    }
}