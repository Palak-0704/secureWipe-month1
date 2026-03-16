use axum::{
    extract::Path,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;

use super::anomaly_detector::detect_offline_result_anomalies;
use super::certificate_render::render_certificate_pdf;
use super::certificate_crypto::{payload_sha256, sign_certificate_payload, verify_certificate_payload};
use super::errors::AppError;
use super::storage::{collect_logs_for_wipe_id, read_completion_artifacts};
use super::types::{
    CertificateData, CertificateDevice, CertificateResponse, CertificateVerifyRequest,
    CertificateReviewResponse, CertificateVerifyResponse, CompletionArtifacts, LogsResponse,
    SessionArtifactsResponse, CURRENT_OFFLINE_RESULT_SCHEMA_VERSION,
    CURRENT_WIPE_MANIFEST_SCHEMA_VERSION,
};

fn certificate_status_from_artifacts(artifacts: &CompletionArtifacts) -> (String, String) {
    if artifacts.result.verification_passed && artifacts.result.completion_status.is_verified() {
        return (
            "Verified Completed".to_string(),
            "Low (validated typed artifacts)".to_string(),
        );
    }

    if artifacts.result.completion_status.requires_review() {
        return (
            "Manual Review Required".to_string(),
            "Review required (typed artifacts indicate incomplete verification evidence)".to_string(),
        );
    }

    (
        "Verification Failed".to_string(),
        "High (typed artifacts indicate failure)".to_string(),
    )
}

fn load_validated_completion_artifacts(session_id: &str) -> Result<CompletionArtifacts, AppError> {
    let Some(artifacts) = read_completion_artifacts(session_id) else {
        return Err(AppError::not_found(
            "session_artifacts_not_found",
            "Session manifest or offline result artifacts were not found.",
        ));
    };

    if artifacts.manifest.schema_version != CURRENT_WIPE_MANIFEST_SCHEMA_VERSION {
        return Err(AppError::conflict(
            "unsupported_session_schema_version",
            format!(
                "Session manifest schema version {} is unsupported; expected version {}.",
                artifacts.manifest.schema_version,
                CURRENT_WIPE_MANIFEST_SCHEMA_VERSION
            ),
        ));
    }

    if artifacts.result.schema_version != CURRENT_OFFLINE_RESULT_SCHEMA_VERSION {
        return Err(AppError::conflict(
            "unsupported_offline_result_schema_version",
            format!(
                "Offline result schema version {} is unsupported; expected version {}.",
                artifacts.result.schema_version,
                CURRENT_OFFLINE_RESULT_SCHEMA_VERSION
            ),
        ));
    }

    let artifact_consistent = artifacts.manifest.session_id == artifacts.result.session_id
        && artifacts.manifest.target_device_id == artifacts.result.target_device_id
        && artifacts.manifest.target_device_model == artifacts.result.target_device_model
        && artifacts.manifest.target_device_size_gb == artifacts.result.target_device_size_gb;

    if !artifact_consistent {
        return Err(AppError::conflict(
            "session_artifact_identity_mismatch",
            "Session manifest and offline result artifacts do not describe the same target device.",
        ));
    }

    Ok(artifacts)
}

fn build_certificate_data_from_artifacts(wipe_id: &str, artifacts: &CompletionArtifacts) -> CertificateData {
    let (status, recovery_risk) = certificate_status_from_artifacts(artifacts);
    CertificateData {
        wipe_id: wipe_id.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        mode: artifacts.manifest.mode.clone(),
        method: artifacts.manifest.method.clone(),
        status,
        recovery_risk,
        devices: vec![CertificateDevice {
            id: artifacts.result.target_device_id.clone(),
            model: artifacts.result.target_device_model.clone(),
        }],
        log_count: collect_logs_for_wipe_id(wipe_id).len(),
    }
}

fn build_certificate_data_from_logs(wipe_id: &str) -> Result<CertificateData, AppError> {
    let logs = collect_logs_for_wipe_id(wipe_id);
    if logs.is_empty() {
        return Err(AppError::not_found(
            "certificate_source_not_found",
            "No wipe records found for this wipe/session ID.",
        ));
    }

    let mut devices = vec![];
    let mut method = "unknown".to_string();
    for entry in &logs {
        if entry.recommendation != "-" {
            method = entry.recommendation.clone();
        }
        if entry.device_id != "-" {
            devices.push(CertificateDevice {
                id: entry.device_id.clone(),
                model: entry.model.clone(),
            });
        }
    }

    Ok(CertificateData {
        wipe_id: wipe_id.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        mode: "handoff-first".to_string(),
        method,
        status: "Prepared/Simulated".to_string(),
        recovery_risk: "Low (simulation evidence only)".to_string(),
        devices,
        log_count: logs.len(),
    })
}

fn build_certificate_response(wipe_id: &str) -> Result<CertificateResponse, AppError> {
    let cert = match load_validated_completion_artifacts(wipe_id) {
        Ok(artifacts) => build_certificate_data_from_artifacts(wipe_id, &artifacts),
        Err(err) => {
            if err.code == "session_artifacts_not_found" {
                build_certificate_data_from_logs(wipe_id)?
            } else {
                return Err(err);
            }
        }
    };

    let cert_json = serde_json::to_value(&cert).unwrap_or_else(|_| serde_json::json!({}));
    let signature = sign_certificate_payload(&cert_json)?;
    let signature_sha256 = signature.payload_sha256.clone();

    Ok(CertificateResponse {
        certificate: cert_json,
        signature_sha256,
        signature,
    })
}

fn build_certificate_review(wipe_id: &str) -> Result<CertificateReviewResponse, AppError> {
    let artifacts = load_validated_completion_artifacts(wipe_id)?;
    let certificate = build_certificate_response(wipe_id)?;
    let signature_verified = verify_certificate_payload(
        &certificate.certificate,
        &certificate.signature.signature_base64,
        &certificate.signature.public_key_base64,
    )?;

    let mut issues = Vec::new();
    if !artifacts.result.verification_passed {
        issues.push("Offline verification did not pass.".to_string());
    }
    if artifacts.result.completion_status.requires_review() {
        issues.push("Offline completion evidence requires manual review.".to_string());
    }
    if let Some(notes) = artifacts.result.verification_notes.as_ref() {
        if !notes.trim().is_empty() {
            issues.push(format!("Verification notes: {}", notes.trim()));
        }
    }
    if !signature_verified {
        issues.push("Generated certificate signature could not be verified.".to_string());
    }
    if artifacts.result.completion_status.is_verified() && artifacts.result.verification_evidence.is_none() {
        issues.push("Verified result has no structured verification evidence attached.".to_string());
    }

    let anomaly_alerts = detect_offline_result_anomalies(
        artifacts.result.verification_passed,
        &artifacts.result.completion_status,
        artifacts.result.verification_notes.as_deref(),
        artifacts.result.verification_evidence.as_ref(),
    );
    for anomaly in &anomaly_alerts {
        issues.push(anomaly.clone());
    }

    let certificate_eligible = artifacts.result.verification_passed
        && artifacts.result.completion_status.is_verified()
        && artifacts.result.verification_evidence.is_some()
        && signature_verified
        && anomaly_alerts.is_empty();

    Ok(CertificateReviewResponse {
        status: if certificate_eligible {
            "certificate_review_ready".to_string()
        } else {
            "certificate_review_attention_required".to_string()
        },
        wipe_id: wipe_id.to_string(),
        manifest_phase: artifacts.manifest.phase,
        completion_status: artifacts.result.completion_status,
        verification_passed: artifacts.result.verification_passed,
        certificate_eligible,
        signature_verified,
        recommended_action: if certificate_eligible {
            "Certificate may be distributed to users or downstream systems.".to_string()
        } else {
            "Review completion artifacts and verification notes before distributing this certificate.".to_string()
        },
        issues,
        verification_evidence: artifacts.result.verification_evidence,
    })
}

pub async fn get_session_artifacts(
    Path(session_id): Path<String>,
) -> Result<Json<SessionArtifactsResponse>, AppError> {
    let artifacts = load_validated_completion_artifacts(&session_id)?;

    Ok(Json(SessionArtifactsResponse {
        status: "session_artifacts_ready".to_string(),
        session_id,
        manifest_phase: artifacts.manifest.phase,
        manifest_schema_version: artifacts.manifest.schema_version,
        result_schema_version: artifacts.result.schema_version,
        verification_passed: artifacts.result.verification_passed,
        completion_status: artifacts.result.completion_status,
        target_device_id: artifacts.result.target_device_id,
        target_device_model: artifacts.result.target_device_model,
        target_device_size_gb: artifacts.result.target_device_size_gb,
        artifact_consistent: true,
    }))
}

pub async fn get_certificate(Path(wipe_id): Path<String>) -> Result<Json<CertificateResponse>, AppError> {
    Ok(Json(build_certificate_response(&wipe_id)?))
}

pub async fn get_certificate_review(
    Path(wipe_id): Path<String>,
) -> Result<Json<CertificateReviewResponse>, AppError> {
    Ok(Json(build_certificate_review(&wipe_id)?))
}

pub async fn get_certificate_pdf(Path(wipe_id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let review = build_certificate_review(&wipe_id)?;
    let certificate = build_certificate_response(&wipe_id)?;
    let pdf = render_certificate_pdf(&review, &certificate.certificate);
    let file_name = format!("securewipe-certificate-{}.pdf", wipe_id);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("application/pdf"))
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file_name)).map_err(|_| {
                AppError::internal_server_error(
                    "certificate_pdf_filename_invalid",
                    "Failed to build certificate PDF filename.",
                )
            })?,
        )
        .body(axum::body::Body::from(pdf))
        .map_err(|_| {
            AppError::internal_server_error(
                "certificate_pdf_response_failed",
                "Failed to build certificate PDF response.",
            )
        })?;

    Ok(response.into_response())
}

pub async fn verify_certificate(
    Json(request): Json<CertificateVerifyRequest>,
) -> Result<Json<CertificateVerifyResponse>, AppError> {
    let algorithm = "ed25519".to_string();
    let payload_sha256 = payload_sha256(&request.certificate)?;
    let verified = verify_certificate_payload(
        &request.certificate,
        &request.signature_base64,
        &request.public_key_base64,
    )?;

    Ok(Json(CertificateVerifyResponse {
        status: if verified {
            "certificate_verified".to_string()
        } else {
            "certificate_verification_failed".to_string()
        },
        algorithm,
        verified,
        payload_sha256,
    }))
}

pub async fn get_logs(Path(wipe_id): Path<String>) -> Result<Json<LogsResponse>, AppError> {
    let logs = collect_logs_for_wipe_id(&wipe_id)
        .into_iter()
        .map(|e| {
            format!(
                "[{}] {} ({}) [op:{}]",
                e.timestamp,
                e.explanation,
                e.phase.unwrap_or_default(),
                e.operation_id.unwrap_or_else(|| "unknown".to_string())
            )
        })
        .collect::<Vec<_>>();

    if logs.is_empty() {
        return Err(AppError::not_found(
            "wipe_logs_not_found",
            "No logs found for provided wipe/session ID.",
        ));
    }

    Ok(Json(LogsResponse { logs }))
}