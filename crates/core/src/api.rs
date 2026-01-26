// Add: Device scan event logging endpoint
async fn log_device_scan() -> Json<serde_json::Value> {
    let entry = serde_json::json!({
        "device_id": "-",
        "model": "-",
        "recommendation": "-",
        "explanation": "Device scan completed.",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    let path = "data/feedback_history.json";
    let history: Vec<serde_json::Value> = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(Vec::new);
    let mut history = history;
    history.push(entry);
    let _ = std::fs::create_dir_all("data");
    let _ = std::fs::write(path, serde_json::to_string_pretty(&history).unwrap_or("[]".to_string()));
    Json(serde_json::json!({"status": "scan_logged"}))
}
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
    use crate::devices::get_current_device_state;
    let devices = get_current_device_state().unwrap_or_else(|_| vec![]);
    if devices.is_empty() {
        return Json(SystemHealthResponse {
            health: "".to_string(),
            update_available: false,
        });
    }
    let mut health = "Good".to_string();
    let update_available = false;
    for d in &devices {
        if let Some(err) = &d.error {
            if !err.is_empty() {
                health = "Warning".to_string();
            }
        }
        if let Some(smart) = &d.smart_status {
            if smart.to_lowercase() != "ok" && smart.to_lowercase() != "passed" {
                health = "Warning".to_string();
            }
        }
    }
    Json(SystemHealthResponse {
        health,
        update_available,
    })
}

// --- Security Status Handler ---
async fn get_security_status() -> Json<SecurityStatusResponse> {
    use crate::devices::get_current_device_state;
    let devices = get_current_device_state().unwrap_or_else(|_| vec![]);
    if devices.is_empty() {
        return Json(SecurityStatusResponse {
            status: "".to_string(),
            protections_active: false,
        });
    }
    let mut status = "Secure".to_string();
    let mut protections_active = true;
    for d in &devices {
        if d.removable == Some(true) {
            protections_active = false;
            status = "Some Removable".to_string();
        }
        if let Some(err) = &d.error {
            if !err.is_empty() {
                protections_active = false;
                status = "Warning".to_string();
            }
        }
    }
    Json(SecurityStatusResponse {
        status,
        protections_active,
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
    let mut devices = crate::devices::detect_devices();

    // Load feedback history
    let path = "data/feedback_history.json";
    let history: Vec<serde_json::Value> = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(Vec::new);

    // Find the most recent scan event timestamp
    let last_scan_ts = history.iter()
        .filter_map(|entry| {
            let explanation = entry.get("explanation")?.as_str()?;
            if explanation.to_lowercase().contains("scan completed") {
                entry.get("timestamp")?.as_str()
            } else {
                None
            }
        })
        .max()
        .map(|s| s.to_string());

    // Only filter out devices that have a wipe event AFTER the most recent scan event
    let mut wiped_ids = std::collections::HashSet::new();
    for entry in &history {
        let explanation = entry.get("explanation").and_then(|v| v.as_str()).unwrap_or("");
        if explanation.to_lowercase().contains("wipe completed") {
            let ts = entry.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            let device_id = entry.get("device_id").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(ref scan_ts) = last_scan_ts {
                if ts > scan_ts.as_str() {
                    wiped_ids.insert(device_id.to_string());
                }
            } else {
                wiped_ids.insert(device_id.to_string());
            }
        }
    }
    // Debug: print all wiped_ids
    println!("[DEBUG] Wiped device IDs (after last scan): {:#?}", wiped_ids);
    for d in &devices {
        if wiped_ids.contains(&d.id) {
            println!("[DEBUG] Filtering out device: {} (matched wiped_ids)", d.id);
        } else {
            println!("[DEBUG] Keeping device: {}", d.id);
        }
    }
    devices.retain(|d| !wiped_ids.contains(&d.id));
    println!("[DEBUG] /api/devices returns: {:#?}", devices);
    Json(devices)
}

async fn start_wipe(Json(req): Json<WipeRequest>) -> Json<WipeStartResponse> {
    // Simulate wipe for each device (real logic would be async and track progress)
    let wipe_id = "wipe123".to_string();
    let devices = detect_devices();
    let mut new_entries = Vec::new();
    for id in &req.device_ids {
        if let Some(device) = devices.iter().find(|d| &d.id == id) {
            let _result = perform_wipe(device);
            // Append wipe entry to feedback_history.json
            let entry = serde_json::json!({
                "device_id": device.id,
                "model": device.model,
                "recommendation": req.method.clone(),
                "explanation": "Simulated wipe completed.",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });
            new_entries.push(entry);
        }
    }
    // Load existing history
    let path = "data/feedback_history.json";
    let mut history: Vec<serde_json::Value> = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(Vec::new);
    history.extend(new_entries);
    // Save updated history
    let _ = std::fs::create_dir_all("data");
    let _ = std::fs::write(path, serde_json::to_string_pretty(&history).unwrap_or("[]".to_string()));
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
        .route("/api/scan/log", post(log_device_scan))
}

// Handler for root path
async fn root_handler() -> &'static str {
    "SecureWipe-AI backend is running. See /api/* for endpoints."
}


