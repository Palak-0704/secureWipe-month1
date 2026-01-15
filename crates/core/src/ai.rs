
//! src/ai.rs
//!
//! AI-Powered Smart Erasure Advisor (Month 1 Submission)
//! - Input: crate::devices::Device, user profile, compliance requirements
//! - Output: Recommendation with method, estimated_minutes, risk_level, explanation, confidence, compliance_notes.

use crate::devices::Device;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComplianceContext {
    pub gdpr: bool,
    pub hipaa: bool,
    pub nist: bool,
    pub custom: Option<String>,
}

/// Public API: recommend a wipe method for given device and compliance context.
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
    /// Example chatbot hook for AI advisor
    pub fn chatbot_recommend(device: &Device) -> String {
        let rec = recommend_method(device, None);
        format!("Recommended wipe method: {} ({} mins, risk: {}, confidence: {:.2}). {}", rec.method, rec.estimated_minutes, rec.risk_level, rec.confidence, rec.explanation)
    }
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

    match dtype.as_str() {
        "NVME" => Recommendation::new(
            "nvme-sanitize",
            15,
            "low",
            "NVMe drives typically support sanitize commands which are effective for secure erase.",
            0.85,
            compliance_notes,
        ),
        "SSD" => {
            if device.encrypted {
                Recommendation::new(
                    "crypto-erase",
                    5,
                    "low",
                    "Device reports encryption; destroying keys (crypto-erase) is fastest and low-risk for SSDs.",
                    0.9,
                    compliance_notes,
                )
            } else {
                Recommendation::new(
                    "ata-secure-erase",
                    20,
                    "low",
                    "ATA Secure Erase (or vendor secure-erase) is preferred for SSDs to avoid wear and ensure full erase.",
                    0.75,
                    compliance_notes,
                )
            }
        }
        "HDD" => {
            let est = estimate_overwrite_time_minutes(device.size_gb);
            Recommendation::new(
                "overwrite",
                est,
                "low",
                "Multi-pass overwrite reduces recoverability on magnetic media (HDD).",
                0.8,
                compliance_notes,
            )
        }
        "USB" | "REMOVABLE" => {
            if device.encrypted {
                Recommendation::new(
                    "crypto-erase",
                    5,
                    "low",
                    "Encrypted removable media: destroying keys or factory-reset where supported.",
                    0.8,
                    compliance_notes,
                )
            } else {
                Recommendation::new(
                    "overwrite",
                    10,
                    "medium",
                    "Overwrite contents of removable media. For some devices, factory-reset is preferred.",
                    0.6,
                    compliance_notes,
                )
            }
        }
        "PHONE" => Recommendation::new(
            "overwrite",
            10,
            "medium",
            "Phones: recommend platform factory-reset or vendor secure wipe where available; otherwise overwrite user partitions.",
            0.65,
            compliance_notes,
        ),
        _ => {
            if device.encrypted {
                Recommendation::new(
                    "crypto-erase",
                    5,
                    "low",
                    "Device indicates encryption; crypto-erase (key destruction) is most efficient.",
                    0.7,
                    compliance_notes,
                )
            } else {
                Recommendation::new(
                    "overwrite",
                    15,
                    "medium",
                    "Generic device: use overwrite as a conservative approach.",
                    0.6,
                    compliance_notes,
                )
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{Device};

    #[test]
    fn chatbot_qa_answers() {
        assert_eq!(chatbot_qa("what is secure wipe?"), "A secure wipe is a process that ensures all data on a device is irrecoverably erased.");
        assert_eq!(chatbot_qa("how do I wipe an ssd?"), "For SSDs, use crypto-erase if encrypted, or ATA Secure Erase if supported.");
        assert_eq!(chatbot_qa("what is risk score?"), "Risk score estimates the chance that data could be recovered after wiping. Lower is better.");
        assert_eq!(chatbot_qa("unknown question"), "Sorry, I don't know the answer to that yet. Please consult the documentation.");
    }

    #[test]
    fn encrypted_device_prefers_crypto_erase() {
        let d = Device {
            id: "t".into(),
            dev_type: "SSD".into(),
            model: "m".into(),
            serial: None,
            size_gb: 256,
            encrypted: true,
            hpa_dco: false,
            firmware: None,
            metadata: Default::default(),
        };
        let ctx = ComplianceContext { gdpr: false, hipaa: true, nist: true, custom: None };
        let rec = recommend_method(&d, Some(&ctx));
        assert_eq!(rec.method, "crypto-erase");
        assert!(rec.confidence > 0.75);
        assert!(rec.compliance_notes.is_some());
    }
}
