use serde_json::Value;

use super::types::CertificateReviewResponse;

pub fn render_certificate_pdf(review: &CertificateReviewResponse, certificate: &Value) -> Vec<u8> {
    let devices = certificate
        .get("devices")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|device| {
                    let id = device
                        .get("id")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown-id");
                    let model = device
                        .get("model")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown-model");
                    format!("{} ({})", id, model)
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "none".to_string());

    let mut lines = vec![
        format!("SecureWipe Certificate: {}", review.wipe_id),
        format!(
            "Generated: {}",
            certificate
                .get("generated_at")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
        ),
        format!(
            "Mode: {}",
            certificate
                .get("mode")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
        ),
        format!(
            "Method: {}",
            certificate
                .get("method")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
        ),
        format!(
            "Status: {}",
            certificate
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
        ),
        format!(
            "Recovery Risk: {}",
            certificate
                .get("recovery_risk")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
        ),
        format!("Devices: {}", devices),
        format!("Manifest Phase: {}", review.manifest_phase),
        format!("Completion Status: {}", review.completion_status),
        format!("Verification Passed: {}", review.verification_passed),
    ];

    // Include structured verification evidence when present.
    if let Some(ev) = &review.verification_evidence {
        lines.push(format!(
            "Evidence Sample Blocks Checked: {}",
            ev.sample_blocks_checked
        ));
        lines.push(format!(
            "Evidence Sample Block Anomalies: {}",
            ev.sample_blocks_anomalies
        ));
        if let Some(algo) = &ev.checksum_algorithm {
            lines.push(format!("Evidence Checksum Algorithm: {}", algo));
        }
        if let Some(tool) = &ev.verification_tool {
            lines.push(format!("Evidence Verification Tool: {}", tool));
        }
        if let Some(op) = &ev.operator_id {
            lines.push(format!("Evidence Operator ID: {}", op));
        }
    } else {
        lines.push("Evidence: Not provided.".to_string());
    }

    lines.extend([
        format!("Certificate Eligible: {}", review.certificate_eligible),
        format!("Signature Verified: {}", review.signature_verified),
        format!("Recommended Action: {}", review.recommended_action),
        format!("Issues: {}", join_issues(&review.issues)),
    ]);

    render_lines_as_pdf(&lines)
}

fn join_issues(issues: &[String]) -> String {
    if issues.is_empty() {
        "none".to_string()
    } else {
        issues.join("; ")
    }
}

fn render_lines_as_pdf(lines: &[String]) -> Vec<u8> {
    let mut content = String::from("BT\n/F1 12 Tf\n50 780 Td\n14 TL\n");
    for line in lines {
        content.push_str(&format!("({}) Tj\nT*\n", escape_pdf_text(line)));
    }
    content.push_str("ET\n");

    let objects = vec![
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_string(),
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_string(),
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n".to_string(),
        format!(
            "4 0 obj\n<< /Length {} >>\nstream\n{}endstream\nendobj\n",
            content.len(),
            content
        ),
        "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_string(),
    ];

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for object in &objects {
        offsets.push(pdf.len());
        pdf.push_str(object);
    }

    let xref_offset = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for offset in offsets {
        pdf.push_str(&format!("{:010} 00000 n \n", offset));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_offset
    ));

    pdf.into_bytes()
}

fn escape_pdf_text(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii())
        .flat_map(|c| match c {
            '(' => "\\(".chars().collect::<Vec<_>>(),
            ')' => "\\)".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' | '\r' => " ".chars().collect::<Vec<_>>(),
            _ => vec![c],
        })
        .collect()
}