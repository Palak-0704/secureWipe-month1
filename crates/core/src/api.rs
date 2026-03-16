use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};
use crate::wipe_history;

pub mod errors;
mod artifact_handlers;
mod anomaly_detector;
mod auth_middleware;
pub mod certificate_crypto;
mod certificate_render;
mod guards;
mod misc_handlers;
mod offline_handlers;
mod usb_imaging;
pub mod result_policy;
pub mod storage;
pub mod types;
mod wipe_handlers;

use self::guards::high_risk_guard;
use self::artifact_handlers::{
    get_certificate, get_certificate_pdf, get_certificate_review, get_logs,
    get_session_artifacts, verify_certificate,
};
use self::misc_handlers::{
    advisor_recommend, chatbot, get_security_status, get_system_health, list_devices,
    log_device_scan, preflight_mvp_check, root_handler,
};
use self::offline_handlers::{
    create_wipe_session, execute_offline_wipe, get_session_status, ingest_offline_result,
    list_usb_candidates, list_wipe_sessions, prepare_bootable_usb, resume_wipe_session,
};
use self::wipe_handlers::{
    session_progress_stream, start_wipe, wipe_confirm_final, wipe_confirm_init,
    wipe_confirm_risk, wipe_progress,
};

pub fn api_router() -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/api/devices", get(list_devices))
        .route("/api/wipe/confirm-init", post(wipe_confirm_init))
        .route("/api/wipe/confirm-risk", post(wipe_confirm_risk))
        .route("/api/wipe/confirm-final", post(wipe_confirm_final))
        .route("/api/wipe/start", post(start_wipe))
        .route("/api/offline/wipe/execute", post(execute_offline_wipe))
        .route("/api/offline/result/ingest", post(ingest_offline_result))
        .route("/api/wipe/progress/:wipe_id", get(wipe_progress))
        .route("/api/wipe/session/:session_id/progress/stream", get(session_progress_stream))
        .route("/api/advisor/recommend", post(advisor_recommend))
        .route("/api/chatbot", post(chatbot))
        .route("/api/certificate/:wipe_id", get(get_certificate))
        .route("/api/certificate/:wipe_id/review", get(get_certificate_review))
        .route("/api/certificate/:wipe_id/pdf", get(get_certificate_pdf))
        .route("/api/certificate/verify", post(verify_certificate))
        .route("/api/logs/:wipe_id", get(get_logs))
        .route("/api/wipe/history", get(wipe_history::wipe_history))
        .route("/api/system/health", get(get_system_health))
        .route("/api/system/security", get(get_security_status))
        .route("/api/scan/log", post(log_device_scan))
        .route("/api/preflight/mvp", get(preflight_mvp_check))
        .route("/api/usb/devices", get(list_usb_candidates))
        .route("/api/wipe/session/create", post(create_wipe_session))
        .route("/api/wipe/session/:session_id/status", get(get_session_status))
        .route("/api/wipe/session/:session_id/resume", post(resume_wipe_session))
        .route("/api/wipe/session/:session_id/artifacts", get(get_session_artifacts))
        .route("/api/usb/prepare", post(prepare_bootable_usb))
        .route("/api/wipe/sessions", get(list_wipe_sessions))
        .layer(middleware::from_fn(auth_middleware::bearer_token_auth))
        .layer(middleware::from_fn(high_risk_guard))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
}

#[cfg(test)]
mod tests;
