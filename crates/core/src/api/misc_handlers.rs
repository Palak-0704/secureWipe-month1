use axum::Json;
use chrono::Utc;

use crate::ai::{chatbot_groq_api_with_config, recommend_method, ComplianceContext};
use crate::devices::{detect_devices, Device};

use super::errors::AppError;
use super::storage::{append_history, new_operation_id};
use super::types::{
	AdvisorRequest, AdvisorResponse, ChatbotRequest, ChatbotResponse, HistoryEntry,
	MvpPreflightResponse, SecurityStatusResponse, SystemHealthResponse,
};

pub async fn root_handler() -> &'static str {
	"SecureWipe-AI backend is running. See /api/* for endpoints."
}

pub async fn log_device_scan() -> Json<serde_json::Value> {
	append_history(HistoryEntry {
		device_id: "-".to_string(),
		model: "-".to_string(),
		recommendation: "-".to_string(),
		explanation: "Device scan completed.".to_string(),
		timestamp: Utc::now().to_rfc3339(),
		operation_id: Some(new_operation_id()),
		wipe_id: None,
		phase: Some("in_app_scan".to_string()),
	});
	Json(serde_json::json!({ "status": "scan_logged" }))
}

pub async fn list_devices() -> Json<Vec<Device>> {
	Json(detect_devices())
}

pub async fn advisor_recommend(
	Json(req): Json<AdvisorRequest>,
) -> Result<Json<AdvisorResponse>, AppError> {
	if req.device_ids.is_empty() {
		return Err(AppError::unprocessable_entity(
			"device_ids_required",
			"At least one device ID is required for advisor recommendation.",
		));
	}

	let devices = detect_devices();
	let compliance = ComplianceContext {
		gdpr: req.compliance.to_lowercase().contains("gdpr"),
		hipaa: req.compliance.to_lowercase().contains("hipaa"),
		nist: req.compliance.to_lowercase().contains("nist"),
		custom: None,
	};

	if let Some(device) = req
		.device_ids
		.first()
		.and_then(|id| devices.iter().find(|d| &d.id == id))
	{
		let rec = recommend_method(device, Some(&compliance));
		return Ok(Json(AdvisorResponse {
			method: rec.method,
			estimated_minutes: rec.estimated_minutes,
			risk_level: rec.risk_level,
			explanation: rec.explanation,
			confidence: rec.confidence,
			compliance_notes: rec.compliance_notes,
		}));
	}

	Err(AppError::not_found(
		"target_device_not_found",
		"Requested device ID was not found for advisor recommendation.",
	))
}

pub async fn chatbot(Json(req): Json<ChatbotRequest>) -> Result<Json<ChatbotResponse>, AppError> {
	if req.message.trim().is_empty() {
		return Err(AppError::bad_request(
			"message_required",
			"Chatbot message cannot be empty.",
		));
	}
	if req.message.len() > 2000 {
		return Err(AppError::bad_request(
			"message_too_long",
			"Chatbot message must be 2000 characters or fewer.",
		));
	}

	let model = "llama-3.1-8b-instant";
	let system_prompt = "You are SecureWipe AI, an expert in secure data erasure.";
	let concise = req.concise.unwrap_or(true);

	#[cfg(feature = "groq_api")]
	let reply = chatbot_groq_api_with_config(&req.message, model, system_prompt, concise)
		.await
		.map_err(|e| AppError::service_unavailable("chatbot_upstream_error", e))?;

	#[cfg(not(feature = "groq_api"))]
	let reply = chatbot_groq_api_with_config(&req.message, model, system_prompt, concise)
		.map_err(|e| AppError::service_unavailable("chatbot_unavailable", e))?;

	Ok(Json(ChatbotResponse { reply }))
}

pub async fn get_system_health() -> Json<SystemHealthResponse> {
	let devices = detect_devices();
	if devices.is_empty() {
		return Json(SystemHealthResponse {
			health: "".to_string(),
			update_available: false,
		});
	}

	let mut health = "Good".to_string();
	for d in &devices {
		if let Some(err) = &d.error {
			if !err.is_empty() {
				health = "Warning".to_string();
				break;
			}
		}
		if let Some(smart) = &d.smart_status {
			let smart_l = smart.to_lowercase();
			if smart_l != "ok" && smart_l != "passed" {
				health = "Warning".to_string();
				break;
			}
		}
	}

	Json(SystemHealthResponse {
		health,
		update_available: false,
	})
}

pub async fn get_security_status() -> Json<SecurityStatusResponse> {
	let devices = detect_devices();
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
			status = "Some Removable".to_string();
			protections_active = false;
		}
		if let Some(err) = &d.error {
			if !err.is_empty() {
				status = "Warning".to_string();
				protections_active = false;
			}
		}
	}

	Json(SecurityStatusResponse {
		status,
		protections_active,
	})
}

pub async fn preflight_mvp_check() -> Json<MvpPreflightResponse> {
	let host_os = std::env::consts::OS.to_string();
	let mut notes = vec![];
	let mvp_host_ok = host_os == "windows";

	if !mvp_host_ok {
		notes.push("MVP currently supports Windows host application flow only.".to_string());
	}

	notes.push("MVP target profile: single-disk consumer laptop, UEFI boot mode.".to_string());
	notes.push("Secure Boot may require signed boot image in later phase.".to_string());

	Json(MvpPreflightResponse {
		host_os,
		uefi_ready: true,
		secure_boot_supported_in_mvp: false,
		mvp_supported: mvp_host_ok,
		notes,
	})
}

#[cfg(test)]
mod tests {
	use axum::Json;

	use super::{advisor_recommend, chatbot};
	use crate::api::types::{AdvisorRequest, ChatbotRequest};

	#[tokio::test]
	async fn advisor_recommend_rejects_empty_device_ids() {
		let req = AdvisorRequest {
			device_ids: vec![],
			compliance: "gdpr".to_string(),
		};

		let err = advisor_recommend(Json(req)).await.err().expect("expected error");
		assert_eq!(err.code, "device_ids_required");
		assert_eq!(err.status.as_u16(), 422);
	}

	#[tokio::test]
	async fn chatbot_rejects_empty_message() {
		let req = ChatbotRequest {
			message: "   ".to_string(),
			concise: Some(true),
		};

		let err = chatbot(Json(req)).await.err().expect("expected error");
		assert_eq!(err.code, "message_required");
		assert_eq!(err.status.as_u16(), 400);
	}
}
