/// Sanitize user input for chatbot (basic: trim, remove control chars)
///
/// # Arguments
/// * `input` - User input string
///
/// # Returns
/// Sanitized string safe for chatbot API
pub fn sanitize_input(input: &str) -> String {
    input.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}
#[cfg(feature = "groq_api")]
/// Call the Groq API chatbot with a toggle for concise/detailed answers. Returns Result<String, String> for error handling.
pub async fn chatbot_groq_api_with_config(input: &str, model: &str, system_prompt: &str, concise: bool) -> Result<String, String> {
    // Load .env if present
    let _ = dotenvy::dotenv();
    use std::env;
    let api_key = env::var("GROQ_API_KEY").unwrap_or_default();
    if api_key.is_empty()
        || api_key == "YOUR_GROQ_API_KEY"
        || api_key == "REPLACE_WITH_YOUR_GROQ_API_KEY"
    {
        return Err("[ERROR] GROQ_API_KEY is missing or placeholder. Set a valid key in environment.".to_string());
    }
    let endpoint = env::var("GROQ_API_ENDPOINT").unwrap_or_else(|_| "https://api.groq.com/openai/v1/chat/completions".to_string());
    let client = reqwest::Client::new();
    let clean_input = sanitize_input(input);
    let prompt = if concise {
        format!("{} (Respond concisely.)", clean_input)
    } else {
        format!("{} (Provide a detailed answer.)", clean_input)
    };
    let payload = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": prompt }
        ]
    });
    let resp = client.post(&endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send().await;
    match resp {
        Ok(r) => {
            match r.json::<serde_json::Value>().await {
                Ok(json) => {
                    if let Some(content) = json.get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c0| c0.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|v| v.as_str()) {
                        return Ok(content.to_string());
                    }
                    if let Some(answer) = json.get("answer").and_then(|v| v.as_str()) {
                        return Ok(answer.to_string());
                    }
                    let msg = format!("[Groq API] Unexpected response format: {}", json);
                    eprintln!("{}", msg);
                    log::warn!("{}", msg);
                    Err(msg)
                }
                Err(e) => {
                    let msg = format!("[Groq API] JSON parse error: {}", e);
                    eprintln!("{}", msg);
                    log::error!("{}", msg);
                    Err(msg)
                }
            }
        },
        Err(e) => Err(format!("[Groq API] Error: {}", e)),
    }
}

#[cfg(not(feature = "groq_api"))]
pub fn chatbot_groq_api_with_config(_input: &str, _model: &str, _system_prompt: &str, _concise: bool) -> Result<String, String> {
    Err("[ERROR] Groq API integration not enabled. Rebuild with --features groq_api and set GROQ_API_KEY and GROQ_API_ENDPOINT.".to_string())
}

// src/ai.rs
//
// AI-Powered Smart Erasure Advisor (Month 1 Submission)
// - Input: crate::devices::Device, user profile, compliance requirements
// - Output: Recommendation with method, estimated_minutes, risk_level, explanation, confidence, compliance_notes.

use crate::devices::Device;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Human-facing recommendation produced by the advisor.
/// Human-facing recommendation produced by the advisor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recommendation {
    /// canonical method id: "overwrite" | "ata-secure-erase" | "nvme-sanitize" | "crypto-erase"
    pub method: String,
    /// estimated minutes for the recommended method (rough heuristic)
    pub estimated_minutes: u32,
    /// "low" | "medium" | "high" risk associated with the recommended method vs device
    pub risk_level: String,
    /// short human explanation for why this method was chosen
    pub explanation: String,
    /// confidence score 0.0 ..= 1.0 (heuristic)
    pub confidence: f32,
    /// Optional compliance notes (e.g., GDPR, NIST, HIPAA)
    pub compliance_notes: Option<String>,
}

impl Recommendation {
    /// Create a new Recommendation
    pub fn new(method: &str, minutes: u32, risk: &str, explanation: &str, confidence: f32, compliance_notes: Option<String>) -> Self {
        Self {
            method: method.to_string(),
            estimated_minutes: minutes,
            risk_level: risk.to_string(),
            explanation: explanation.to_string(),
            confidence: confidence.clamp(0.0, 1.0),
            compliance_notes,
        }
    }
}

/// Compliance context for recommendations
/// Compliance context for recommendations (GDPR, HIPAA, NIST, custom)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComplianceContext {
    pub gdpr: bool,
    pub hipaa: bool,
    pub nist: bool,
    pub custom: Option<String>,
}

/// Public API: recommend a wipe method for given device and compliance context.
///
/// # Arguments
/// * `device` - Reference to [Device]
/// * `compliance` - Optional reference to [ComplianceContext]
///
/// # Returns
/// [Recommendation] for the device and compliance context
///
/// Rules are intentionally conservative and explainable:
///  - If device reports encryption -> prefer "crypto-erase" (destroy keys)
///  - If NVMe -> prefer "nvme-sanitize"
///  - If SSD (non-NVMe) -> prefer "ata-secure-erase" (if supported), fallback to overwrite
///  - If HDD -> prefer multi-pass overwrite
///  - For removable media / phones -> prefer overwrite or factory-reset equivalents
///  - If compliance context is set, add notes to recommendation
///
pub fn recommend_method(device: &Device, compliance: Option<&ComplianceContext>) -> Recommendation {
    // Gather extra info for explanation and health metrics
    let mut extra_explanation = String::new();
    if let Some(fw) = &device.firmware {
        extra_explanation += &format!(" Device firmware: {}.", fw);
    }
    // SMART status
    if let Some(smart) = device.metadata.get("smart_status") {
        if smart.to_lowercase().contains("fail") {
            extra_explanation += " SMART status indicates device health issues. Risk increased.";
        } else if smart.to_lowercase().contains("ok") {
            extra_explanation += " SMART status is OK.";
        }
    }
    // Wear level (for SSDs)
    if let Some(wear) = device.metadata.get("wear_level") {
        extra_explanation += &format!(" SSD wear level: {}.", wear);
        if let Ok(wear_val) = wear.parse::<u8>() {
            if wear_val > 80 {
                extra_explanation += " High wear: device nearing end of life. Consider replacement.";
            } else if wear_val > 50 {
                extra_explanation += " Moderate wear: device has seen significant use.";
            } else {
                extra_explanation += " Low wear: device health is good.";
            }
        }
    }
    // Error rates (for HDDs/SSDs)
    if let Some(errors) = device.metadata.get("error_rate") {
        extra_explanation += &format!(" Error rate: {}.", errors);
        if let Ok(err_val) = errors.parse::<u32>() {
            if err_val > 1000 {
                extra_explanation += " High error rate: device is unreliable.";
            } else if err_val > 100 {
                extra_explanation += " Moderate error rate: monitor device closely.";
            } else {
                extra_explanation += " Low error rate: device is healthy.";
            }
        }
    }
    // Add explainability for compliance context
    let mut explain_compliance = String::new();
    if let Some(ctx) = compliance {
        let mut rules = vec![];
        if ctx.gdpr { rules.push("GDPR"); }
        if ctx.hipaa { rules.push("HIPAA"); }
        if ctx.nist { rules.push("NIST"); }
        if let Some(_c) = &ctx.custom { rules.push("Custom"); }
        if !rules.is_empty() {
            explain_compliance = format!(" Compliance rules applied: {}.", rules.join(", "));
        }
    }
    // Log the recommendation request with device details and timestamp
    let now = Utc::now();
    let device_json = serde_json::to_string(device).unwrap_or_else(|_| "<device-serialize-error>".to_string());
    // Placeholder for the recommendation, will be set below
    // let mut recommendation: Option<Recommendation> = None;

    let dtype = device.dev_type.to_uppercase();
    let mut notes: Vec<String> = vec![];
    if let Some(ctx) = compliance {
        if ctx.gdpr {
            notes.push("GDPR: Ensure complete data destruction and audit trail.".to_string());
        }
        if ctx.hipaa {
            notes.push("HIPAA: Use NIST-compliant methods for PHI.".to_string());
        }
        if ctx.nist {
            notes.push("NIST: Prefer NIST SP 800-88 methods (crypto-erase, sanitize, overwrite). ".to_string());
        }
        if let Some(c) = &ctx.custom {
            let custom_note = format!("Custom: {}", c);
            notes.push(custom_note);
        }
    }
    let compliance_notes = if notes.is_empty() { None } else { Some(notes.join(" ")) };

    let recommendation = Some(match dtype.as_str() {
        "NVME" => Recommendation::new(
            "nvme-sanitize",
            15,
            "low",
            &format!("NVMe drives typically support sanitize commands which are effective for secure erase.{}{}", extra_explanation, explain_compliance),
            0.85,
            compliance_notes.clone(),
        ),
        "SSD" => {
            if device.encrypted {
                Recommendation::new(
                    "crypto-erase",
                    5,
                    "low",
                    &format!("Device reports encryption; destroying keys (crypto-erase) is fastest and low-risk for SSDs.{}{}", extra_explanation, explain_compliance),
                    0.9,
                    compliance_notes.clone(),
                )
            } else {
                Recommendation::new(
                    "ata-secure-erase",
                    20,
                    "low",
                    &format!("ATA Secure Erase (or vendor secure-erase) is preferred for SSDs to avoid wear and ensure full erase.{}{}", extra_explanation, explain_compliance),
                    0.75,
                    compliance_notes.clone(),
                )
            }
        }
        "HDD" => {
            let est = estimate_overwrite_time_minutes(device.size_gb);
            let mut risk = "low";
            let mut explanation = format!("Multi-pass overwrite reduces recoverability on magnetic media (HDD).{}{}", extra_explanation, explain_compliance);
            if let Some(smart) = device.metadata.get("smart_status") {
                if smart.to_lowercase().contains("fail") {
                    risk = "medium";
                    explanation += " Device health is poor; consider replacing the drive.";
                }
            }
            Recommendation::new(
                "overwrite",
                est,
                risk,
                &explanation,
                0.8,
                compliance_notes.clone(),
            )
        }
        "USB" | "REMOVABLE" => {
            if device.encrypted {
                Recommendation::new(
                    "crypto-erase",
                    5,
                    "low",
                    &format!("Encrypted removable media: destroying keys or factory-reset where supported.{}{}", extra_explanation, explain_compliance),
                    0.8,
                    compliance_notes.clone(),
                )
            } else {
                Recommendation::new(
                    "overwrite",
                    10,
                    "medium",
                    &format!("Overwrite contents of removable media. For some devices, factory-reset is preferred.{}{}", extra_explanation, explain_compliance),
                    0.6,
                    compliance_notes.clone(),
                )
            }
        }
        "PHONE" => Recommendation::new(
            "overwrite",
            10,
            "medium",
            &format!("Phones: recommend platform factory-reset or vendor secure wipe where available; otherwise overwrite user partitions.{}{}", extra_explanation, explain_compliance),
            0.65,
            compliance_notes.clone(),
        ),
        _ => {
            if device.encrypted {
                Recommendation::new(
                    "crypto-erase",
                    5,
                    "low",
                    &format!("Device indicates encryption; crypto-erase (key destruction) is most efficient.{}{}", extra_explanation, explain_compliance),
                    0.7,
                    compliance_notes.clone(),
                )
            } else {
                Recommendation::new(
                    "overwrite",
                    15,
                    "medium",
                    &format!("Generic device: use overwrite as a conservative approach.{}{}", extra_explanation, explain_compliance),
                    0.6,
                    compliance_notes.clone(),
                )
            }
        }
    });

    // Log the recommendation to ai_advisor.log
    if let Some(ref rec) = recommendation {
        let rec_json = serde_json::to_string(rec).unwrap_or_else(|_| "<rec-serialize-error>".to_string());
        let log_line = format!("{} | device: {} | recommendation: {}\n", now.to_rfc3339(), device_json, rec_json);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("ai_advisor.log") {
            let _ = file.write_all(log_line.as_bytes());
        }
    }

    recommendation.unwrap()
}

/// Heuristic: estimate overwrite time in minutes based on size (very rough).
fn estimate_overwrite_time_minutes(size_gb: u64) -> u32 {
    // Simplified heuristic: minutes ≈ size_gb * 0.17
    let minutes = (size_gb as f32 * 0.17).ceil() as u32;
    minutes.clamp(5, 24 * 60) // min 5 mins, max 24 hours
}

/// Example chatbot Q&A prototype for Month 1
pub fn chatbot_qa(input: &str) -> String {
    match input.to_lowercase().as_str() {
        "what is secure wipe?" => "A secure wipe is a process that ensures all data on a device is irrecoverably erased.".to_string(),
        "how do i wipe an ssd?" => "For SSDs, use crypto-erase if encrypted, or ATA Secure Erase if supported.".to_string(),
        "what is risk score?" => "Risk score estimates the chance that data could be recovered after wiping. Lower is better.".to_string(),
        _ => "Sorry, I don't know the answer to that yet. Please consult the documentation.".to_string(),
    }
}

/// Main chatbot implementation: Groq API NLP-based chatbot
/// Usage: chatbot_groq_api_with_config(input, model, system_prompt, concise)
/// See CLI integration for details.

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn chatbot_qa_answers() {
        assert_eq!(chatbot_qa("what is secure wipe?"), "A secure wipe is a process that ensures all data on a device is irrecoverably erased.");
        assert_eq!(chatbot_qa("how do I wipe an ssd?"), "For SSDs, use crypto-erase if encrypted, or ATA Secure Erase if supported.");
        assert_eq!(chatbot_qa("what is risk score?"), "Risk score estimates the chance that data could be recovered after wiping. Lower is better.");
        assert_eq!(chatbot_qa("unknown question"), "Sorry, I don't know the answer to that yet. Please consult the documentation.");
    }
    // Additional tests can be added here
}
