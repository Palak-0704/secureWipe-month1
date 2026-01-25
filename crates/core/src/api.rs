#[derive(Serialize)]
struct SystemHealthResponse {
    health: String,
    update_available: bool,
}

#[derive(Serialize)]
struct SecurityStatusResponse {
    status: String,
    protections_active: bool,
}

// --- System Health Handler ---
async fn get_system_health() -> Json<SystemHealthResponse> {
    // TODO: Replace with real health checks
    Json(SystemHealthResponse {
        health: "Good".to_string(),
        update_available: true,
    })
}

// --- Security Status Handler ---
async fn get_security_status() -> Json<SecurityStatusResponse> {
    // TODO: Replace with real security checks
    Json(SecurityStatusResponse {
        status: "Secure".to_string(),
        protections_active: true,
    })
}
// Axum REST API server for SecureWipe-AI
// Scaffolds endpoints for device, wipe, advisor, chatbot, certificate, and logs
// To be placed in crates/core/src/api.rs

use axum::{routing::{get, post}, Router, Json, extract::Path};
use crate::wipe_history;
use serde::{Deserialize, Serialize};

// --- Types ---
use crate::devices::{Device, detect_devices};
use crate::engine::wipe::perform_wipe;
use crate::ai::{recommend_method, ComplianceContext, chatbot_groq_api_with_config};



#[derive(Deserialize)]
struct WipeRequest {
    device_ids: Vec<String>,
    method: String,
}

#[derive(Serialize)]
struct WipeStartResponse {
    status: String,
    wipe_id: String,
}

#[derive(Serialize)]
struct WipeProgressResponse {
    progress: u8,
    status: String,
}

#[derive(Deserialize)]
struct AdvisorRequest {
    device_ids: Vec<String>,
    compliance: String,
}

#[derive(Serialize)]
struct AdvisorResponse {
    method: String,
    estimated_minutes: u32,
    risk_level: String,
    explanation: String,
    confidence: f32,
    compliance_notes: Option<String>,
}

#[derive(Deserialize)]
struct ChatbotRequest {
    message: String,
    concise: Option<bool>,
}

#[derive(Serialize)]
struct ChatbotResponse {
    reply: String,
}

#[derive(Serialize)]
struct CertificateResponse {
    certificate: String,
}

#[derive(Serialize)]
struct LogsResponse {
    logs: Vec<String>,
}

// --- Handlers (stubs) ---
async fn list_devices() -> Json<Vec<Device>> {
    // Use real device detection logic
    // Device struct now includes allocated_gb (used space in GB)
    let devices = crate::devices::detect_devices();
    Json(devices)
}

async fn start_wipe(Json(req): Json<WipeRequest>) -> Json<WipeStartResponse> {
    // Simulate wipe for each device (real logic would be async and track progress)
    let wipe_id = "wipe123".to_string();
    let devices = detect_devices();
    for id in &req.device_ids {
        if let Some(device) = devices.iter().find(|d| &d.id == id) {
            let _result = perform_wipe(device);
            // In real implementation, store result and generate unique wipe_id
        }
    }
    Json(WipeStartResponse { status: "started".into(), wipe_id })
}

async fn wipe_progress(Path(_wipe_id): Path<String>) -> Json<WipeProgressResponse> {
    Json(WipeProgressResponse { progress: 0, status: "pending".into() })
}

async fn advisor_recommend(Json(req): Json<AdvisorRequest>) -> Json<AdvisorResponse> {
    // Use first device for demo; real impl: support multiple
    let devices = detect_devices();
    let compliance = ComplianceContext {
        gdpr: req.compliance.to_lowercase().contains("gdpr"),
        hipaa: req.compliance.to_lowercase().contains("hipaa"),
        nist: req.compliance.to_lowercase().contains("nist"),
        custom: None,
    };
    if let Some(device) = req.device_ids.get(0).and_then(|id| devices.iter().find(|d| &d.id == id)) {
        let rec = recommend_method(device, Some(&compliance));
        Json(AdvisorResponse {
            method: rec.method,
            estimated_minutes: rec.estimated_minutes,
            risk_level: rec.risk_level,
            explanation: rec.explanation,
            confidence: rec.confidence,
            compliance_notes: rec.compliance_notes,
        })
    } else {
        Json(AdvisorResponse {
            method: "unknown".into(),
            estimated_minutes: 0,
            risk_level: "unknown".into(),
            explanation: "Device not found".into(),
            confidence: 0.0,
            compliance_notes: None,
        })
    }
}

async fn chatbot(Json(req): Json<ChatbotRequest>) -> Json<ChatbotResponse> {
    let model = "llama-3.1-8b-instant";
    let system_prompt = "You are SecureWipe AI, an expert in secure data erasure.";
    let concise = req.concise.unwrap_or(true);
    #[cfg(feature = "groq_api")]
    let reply = match chatbot_groq_api_with_config(&req.message, model, system_prompt, concise).await {
        Ok(r) => r,
        Err(e) => format!("[ERROR] {e}"),
    };
    #[cfg(not(feature = "groq_api"))]
    let reply = match chatbot_groq_api_with_config(&req.message, model, system_prompt, concise) {
        Ok(r) => r,
        Err(e) => format!("[ERROR] {e}"),
    };
    Json(ChatbotResponse { reply })
}

async fn get_certificate(Path(_wipe_id): Path<String>) -> Json<CertificateResponse> {
    Json(CertificateResponse { certificate: "base64cert==".into() })
}

async fn get_logs(Path(_wipe_id): Path<String>) -> Json<LogsResponse> {
    Json(LogsResponse { logs: vec!["log1".into(), "log2".into()] })
}

// --- Router ---
pub fn api_router() -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/api/devices", get(list_devices))
        .route("/api/wipe/start", post(start_wipe))
        .route("/api/wipe/progress/:wipe_id", get(wipe_progress))
        .route("/api/advisor/recommend", post(advisor_recommend))
        .route("/api/chatbot", post(chatbot))
        .route("/api/certificate/:wipe_id", get(get_certificate))
        .route("/api/logs/:wipe_id", get(get_logs))
        .route("/api/wipe/history", get(wipe_history::wipe_history))
    .route("/api/system/health", get(get_system_health))
    .route("/api/system/security", get(get_security_status))
}

// Handler for root path
async fn root_handler() -> &'static str {
    "SecureWipe-AI backend is running. See /api/* for endpoints."
}


