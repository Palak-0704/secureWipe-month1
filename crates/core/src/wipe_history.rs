use axum::{Json, extract::Path};
use serde::Serialize;
use std::fs;
use chrono::Utc;

#[derive(Serialize)]
pub struct WipeHistoryEntry {
    pub device_id: String,
    pub model: String,
    pub recommendation: String,
    pub explanation: String,
    pub timestamp: String,
}

pub async fn wipe_history() -> Json<Vec<WipeHistoryEntry>> {
    // For demo: load from feedback_history.json, add fake timestamps
    let data = fs::read_to_string("data/feedback_history.json").unwrap_or("[]".to_string());
    let entries: Vec<serde_json::Value> = serde_json::from_str(&data).unwrap_or(vec![]);
    let now = chrono::Utc::now().to_rfc3339();
    let mapped = entries.into_iter().map(|e| {
        WipeHistoryEntry {
            device_id: e["device_id"].as_str().unwrap_or("").to_string(),
            model: e["model"].as_str().unwrap_or("").to_string(),
            recommendation: e["recommendation"].as_str().unwrap_or("").to_string(),
            explanation: e["explanation"].as_str().unwrap_or("").to_string(),
            timestamp: now.clone(),
        }
    }).collect();
    Json(mapped)
}
