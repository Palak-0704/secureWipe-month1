use axum::{extract::Path, Json};
use axum::response::sse::{Event, KeepAlive, Sse};
use chrono::Utc;
use futures_util::stream;
use std::convert::Infallible;

use crate::devices::detect_devices;
use crate::engine::wipe::perform_wipe_in_app;

use super::errors::AppError;
use super::storage::{
    append_history, collect_logs_for_wipe_id, identity_matches_device, new_operation_id,
    now_id, read_confirmation_state, read_session_manifest, write_confirmation_state,
};
use super::types::{
    LegacyWipeRequest,
    HistoryEntry, TargetIdentity, WipeConfirmFinalRequest, WipeConfirmFinalResponse,
    WipeConfirmInitRequest, WipeConfirmInitResponse, WipeConfirmRiskRequest,
    WipeConfirmRiskResponse, WipeConfirmationState, WipeFlowState, WipeProgressResponse,
    WipeSessionPhase, StartWipeRequest, WipeRequest, WipeStartResponse,
};

pub async fn wipe_confirm_init(
    Json(req): Json<WipeConfirmInitRequest>,
) -> Result<Json<WipeConfirmInitResponse>, AppError> {
    if req.device_ids.is_empty() {
        return Err(AppError::unprocessable_entity(
            "device_ids_required",
            "At least one target device is required.",
        ));
    }

    let expert_mode = req.expert_mode.unwrap_or(false);
    if req.device_ids.len() > 1 && !expert_mode {
        return Err(AppError::forbidden(
            "unsupported_multi_disk_mvp",
            "MVP policy allows single-disk flow only. Enable expert mode for multi-disk.",
        ));
    }

    let host_os = std::env::consts::OS;
    if host_os != "windows" && !expert_mode {
        return Err(AppError::forbidden(
            "unsupported_host_for_mvp",
            "MVP host flow currently supports Windows only unless expert mode is enabled.",
        ));
    }

    let devices = detect_devices();
    let all_found = req
        .device_ids
        .iter()
        .all(|id| devices.iter().any(|d| &d.id == id));
    if !all_found {
        return Err(AppError::not_found(
            "target_device_not_found",
            "One or more target device IDs were not found.",
        ));
    }

    let target_identities = req
        .device_ids
        .iter()
        .filter_map(|id| devices.iter().find(|d| &d.id == id))
        .map(|d| TargetIdentity {
            id: d.id.clone(),
            model: d.model.clone(),
            size_gb: d.size_gb,
            serial: d.serial.clone(),
        })
        .collect::<Vec<_>>();

    let wipe_id = now_id("wipe");
    let confirmation_token = now_id("confirm");
    let state = WipeConfirmationState {
        wipe_id: wipe_id.clone(),
        confirmation_token: confirmation_token.clone(),
        device_ids: req.device_ids,
        target_identities,
        method: req.method,
        created_at: Utc::now().to_rfc3339(),
        flow_state: WipeFlowState::Initialized,
    };
    write_confirmation_state(&state);

    append_history(HistoryEntry {
        device_id: "-".to_string(),
        model: "-".to_string(),
        recommendation: state.method.clone(),
        explanation: "Wipe confirmation initialized.".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        operation_id: Some(new_operation_id()),
        wipe_id: Some(wipe_id.clone()),
        phase: Some("confirm_init".to_string()),
    });

    Ok(Json(WipeConfirmInitResponse {
        status: "confirmation_initialized".to_string(),
        wipe_id,
        confirmation_token,
    }))
}

pub async fn wipe_confirm_risk(
    Json(req): Json<WipeConfirmRiskRequest>,
) -> Result<Json<WipeConfirmRiskResponse>, AppError> {
    let Some(mut state) = read_confirmation_state(&req.wipe_id) else {
        return Err(AppError::not_found(
            "confirmation_not_found",
            "Confirmation state for wipe ID was not found.",
        ));
    };

    if state.confirmation_token != req.confirmation_token {
        return Err(AppError::forbidden(
            "invalid_confirmation_token",
            "Confirmation token is invalid.",
        ));
    }

    if state.flow_state != WipeFlowState::Initialized {
        return Err(AppError::conflict(
            "invalid_confirmation_state",
            "Risk acknowledgement can only occur from initialized state.",
        ));
    }

    if !req.acknowledged {
        return Err(AppError::unprocessable_entity(
            "risk_not_acknowledged",
            "Risk acknowledgement must be true before proceeding.",
        ));
    }

    state.flow_state = WipeFlowState::RiskAcknowledged;
    write_confirmation_state(&state);
    append_history(HistoryEntry {
        device_id: "-".to_string(),
        model: "-".to_string(),
        recommendation: state.method,
        explanation: "Wipe risk acknowledged.".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        operation_id: Some(new_operation_id()),
        wipe_id: Some(req.wipe_id),
        phase: Some("confirm_risk".to_string()),
    });

    Ok(Json(WipeConfirmRiskResponse {
        status: "risk_acknowledged".to_string(),
    }))
}

pub async fn wipe_confirm_final(
    Json(req): Json<WipeConfirmFinalRequest>,
) -> Result<Json<WipeConfirmFinalResponse>, AppError> {
    let Some(mut state) = read_confirmation_state(&req.wipe_id) else {
        return Err(AppError::not_found(
            "confirmation_not_found",
            "Confirmation state for wipe ID was not found.",
        ));
    };

    if state.confirmation_token != req.confirmation_token {
        return Err(AppError::forbidden(
            "invalid_confirmation_token",
            "Confirmation token is invalid.",
        ));
    }

    if state.flow_state != WipeFlowState::RiskAcknowledged {
        return Err(AppError::conflict(
            "risk_step_required",
            "Risk acknowledgement step must be completed first.",
        ));
    }

    if req.confirmation_text.trim().to_uppercase() != "ERASE" {
        return Err(AppError::unprocessable_entity(
            "invalid_final_confirmation_text",
            "Final confirmation text must be ERASE.",
        ));
    }

    state.flow_state = WipeFlowState::FinalConfirmed;
    write_confirmation_state(&state);
    append_history(HistoryEntry {
        device_id: "-".to_string(),
        model: "-".to_string(),
        recommendation: state.method,
        explanation: "Wipe final confirmation accepted (ERASE).".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        operation_id: Some(new_operation_id()),
        wipe_id: Some(req.wipe_id),
        phase: Some("confirm_final".to_string()),
    });

    Ok(Json(WipeConfirmFinalResponse {
        status: "final_confirmation_accepted".to_string(),
    }))
}

fn start_wipe_legacy(req: LegacyWipeRequest) -> Result<WipeStartResponse, AppError> {
    if req.device_ids.is_empty() {
        return Err(AppError::unprocessable_entity(
            "device_ids_required",
            "At least one target device is required.",
        ));
    }

    let wipe_id = now_id("wipe");
    let devices = detect_devices();

    for id in &req.device_ids {
        if let Some(device) = devices.iter().find(|d| &d.id == id) {
            let result = perform_wipe_in_app(device);
            append_history(HistoryEntry {
                device_id: device.id.clone(),
                model: device.model.clone(),
                recommendation: req.method.clone(),
                explanation: result.message,
                timestamp: Utc::now().to_rfc3339(),
                operation_id: Some(new_operation_id()),
                wipe_id: Some(wipe_id.clone()),
                phase: Some("in_app_simulation_legacy".to_string()),
            });
        }
    }

    Ok(WipeStartResponse {
        status: "started_simulation_mode".to_string(),
        wipe_id,
    })
}

fn start_wipe_confirmed(req: WipeRequest) -> Result<WipeStartResponse, AppError> {
    let Some(mut state) = read_confirmation_state(&req.wipe_id) else {
        return Err(AppError::not_found(
            "confirmation_not_found",
            "Confirmation state for wipe ID was not found.",
        ));
    };

    if state.confirmation_token != req.confirmation_token {
        return Err(AppError::forbidden(
            "invalid_confirmation_token",
            "Confirmation token is invalid.",
        ));
    }

    if state.flow_state != WipeFlowState::FinalConfirmed {
        return Err(AppError::conflict(
            "confirmation_incomplete",
            "All confirmation steps must be completed before wipe start.",
        ));
    }

    let wipe_id = state.wipe_id.clone();

    let devices = detect_devices();

    for identity in &state.target_identities {
        let Some(current) = devices.iter().find(|d| d.id == identity.id) else {
            return Err(AppError::conflict(
                "target_device_missing_after_confirmation",
                "Target device disappeared after confirmation; restart confirmation flow.",
            ));
        };
        if !identity_matches_device(identity, current) {
            return Err(AppError::conflict(
                "target_device_identity_mismatch",
                "Target device identity changed after confirmation; restart confirmation flow.",
            ));
        }
    }

    state.flow_state = WipeFlowState::StartedSimulation;
    write_confirmation_state(&state);

    for id in &state.device_ids {
        if let Some(device) = devices.iter().find(|d| &d.id == id) {
            let result = perform_wipe_in_app(device);
            append_history(HistoryEntry {
                device_id: device.id.clone(),
                model: device.model.clone(),
                recommendation: state.method.clone(),
                explanation: result.message,
                timestamp: Utc::now().to_rfc3339(),
                operation_id: Some(new_operation_id()),
                wipe_id: Some(wipe_id.clone()),
                phase: Some("in_app_simulation".to_string()),
            });
        }
    }

    state.flow_state = WipeFlowState::CompletedSimulation;
    write_confirmation_state(&state);

    Ok(WipeStartResponse {
        status: "started_simulation_mode".to_string(),
        wipe_id,
    })
}

pub async fn start_wipe(Json(req): Json<StartWipeRequest>) -> Result<Json<WipeStartResponse>, AppError> {
    let response = match req {
        StartWipeRequest::Confirmed(confirmed) => start_wipe_confirmed(confirmed)?,
        StartWipeRequest::Legacy(legacy) => start_wipe_legacy(legacy)?,
    };

    Ok(Json(response))
}

pub async fn wipe_progress(Path(wipe_id): Path<String>) -> Result<Json<WipeProgressResponse>, AppError> {
    let logs = collect_logs_for_wipe_id(&wipe_id);
    if logs.is_empty() {
        return Err(AppError::not_found(
            "wipe_id_not_found",
            "No wipe records found for provided wipe ID.",
        ));
    }
    Ok(Json(WipeProgressResponse {
        progress: 100,
        status: "completed".to_string(),
    }))
}

/// SSE endpoint that streams session phase/progress updates every 500 ms.
///
/// The stream terminates naturally once the session reaches a terminal phase
/// (completed, failed, verified, certified) so the browser `EventSource`
/// connection closes cleanly without needing an explicit disconnect call.
///
/// Events are named `progress` and carry a JSON payload:
/// ```json
/// { "session_id": "…", "phase": "…", "progress_percent": 42,
///   "resume_required": false, "resume_hint": null, "done": false }
/// ```
pub async fn session_progress_stream(
    Path(session_id): Path<String>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let s = stream::unfold((session_id, false), |(sid, was_done)| async move {
        if was_done {
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let (phase, pct, resume_required, resume_hint, done) =
            match read_session_manifest(&sid) {
                Some(m) => {
                    let done = matches!(
                        m.phase,
                        WipeSessionPhase::Completed
                            | WipeSessionPhase::Failed
                            | WipeSessionPhase::Verified
                            | WipeSessionPhase::Certified
                    );
                    (
                        m.phase.to_string(),
                        m.progress_percent,
                        m.resume_required,
                        m.resume_hint,
                        done,
                    )
                }
                None => ("not_found".to_string(), 0u8, false, None, true),
            };
        let payload = serde_json::json!({
            "session_id": &sid,
            "phase": phase,
            "progress_percent": pct,
            "resume_required": resume_required,
            "resume_hint": resume_hint,
            "done": done,
        });
        let ev = Event::default()
            .event("progress")
            .data(payload.to_string());
        Some((Ok::<Event, Infallible>(ev), (sid, done)))
    });

    Sse::new(s).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}
