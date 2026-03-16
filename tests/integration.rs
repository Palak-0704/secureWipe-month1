#[test]
fn test_edgecase_huge_drive() {
    let d = Device {
        id: "huge1".into(),
        dev_type: "HDD".into(),
        model: "BigDisk500TB".into(),
        serial: Some("BIG500TB".into()),
        size_gb: 500_000,
        allocated_gb: None,
        partitions: vec![],
        connection: None,
        removable: Some(false),
        is_system: Some(false),
        smart_status: Some("OK".to_string()),
        temperature_c: None,
        encrypted: false,
        hpa_dco: false,
        firmware: Some("FW9.9".into()),
        error: None,
        metadata: HashMap::new(),
        detection_confidence: Default::default(),
    };
    let ctx = ComplianceContext { gdpr: false, hipaa: false, nist: true, custom: None };
    let rec = recommend_method(&d, Some(&ctx));
    assert!(rec.estimated_minutes > 1000);
    assert!(rec.explanation.contains("large"));
}

#[test]
fn test_invalid_device_missing_fields() {
    let d = Device {
        id: "invalid1".into(),
        dev_type: "".into(),
        model: "".into(),
        serial: None,
        size_gb: 0,
        allocated_gb: None,
        partitions: vec![],
        connection: None,
        removable: None,
        is_system: None,
        smart_status: None,
        temperature_c: None,
        encrypted: false,
        hpa_dco: false,
        firmware: None,
        error: None,
        metadata: HashMap::new(),
        detection_confidence: Default::default(),
    };
    let ctx = ComplianceContext { gdpr: false, hipaa: false, nist: false, custom: None };
    let rec = recommend_method(&d, Some(&ctx));
    assert_eq!(rec.method, "overwrite");
    assert!(rec.explanation.contains("unknown") || rec.explanation.contains("default"));
}
// Integration tests for SecureWipe-AI (Month 1)
// Add test cases as needed for backend, AI/ML, and security modules.

#[test]
fn dummy_test() {
    assert_eq!(2 + 2, 4);
}

use securewipe_core::ai::{recommend_method, ComplianceContext};
use securewipe_core::devices::Device;
use std::collections::HashMap;

#[test]
fn test_edgecase_usb_encrypted() {
    let d = Device {
        id: "usb1".into(),
        dev_type: "USB".into(),
        model: "EdgeUSB".into(),
        serial: Some("1234".into()),
        size_gb: 8,
        allocated_gb: None,
        partitions: vec![],
        connection: None,
        removable: Some(true),
        is_system: Some(false),
        smart_status: Some("OK".to_string()),
        temperature_c: None,
        encrypted: true,
        hpa_dco: false,
        firmware: Some("FW1.2".into()),
        error: None,
        metadata: HashMap::new(),
        detection_confidence: Default::default(),
    };
    let ctx = ComplianceContext { gdpr: true, hipaa: false, nist: false, custom: None };
    let rec = recommend_method(&d, Some(&ctx));
    assert_eq!(rec.method, "crypto-erase");
    assert!(rec.explanation.contains("encrypted"));
}

#[test]
fn test_edgecase_phone() {
    let d = Device {
        id: "ph1".into(),
        dev_type: "PHONE".into(),
        model: "EdgePhone".into(),
        serial: None,
        size_gb: 64,
        allocated_gb: None,
        partitions: vec![],
        connection: None,
        removable: Some(false),
        is_system: Some(false),
        smart_status: None,
        temperature_c: None,
        encrypted: false,
        hpa_dco: false,
        firmware: None,
        error: None,
        metadata: HashMap::new(),
        detection_confidence: Default::default(),
    };
    let ctx = ComplianceContext { gdpr: false, hipaa: true, nist: true, custom: None };
    let rec = recommend_method(&d, Some(&ctx));
    assert_eq!(rec.method, "overwrite");
    assert!(rec.explanation.contains("Phones"));
}
