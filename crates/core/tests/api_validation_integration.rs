use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hyper::body::to_bytes;
use securewipe_core::api_router;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

fn env_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn next_client_identity() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!(
        "integration-client-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn ensure_test_data_dir() -> &'static str {
    static DATA_DIR: OnceLock<String> = OnceLock::new();
    DATA_DIR.get_or_init(|| {
        let root = std::env::temp_dir().join(format!(
            "securewipe-api-validation-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("failed to create integration test data root");
        unsafe {
            std::env::set_var("SECUREWIPE_DATA_DIR", &root);
        }
        root.to_string_lossy().into_owned()
    })
}

fn test_data_path(parts: &[&str]) -> String {
    let mut path = std::path::PathBuf::from(ensure_test_data_dir());
    for part in parts {
        path.push(part);
    }
    path.to_string_lossy().into_owned()
}

async fn post_json(path: &str, json: &str) -> (StatusCode, serde_json::Value) {
    let _ = ensure_test_data_dir();
    let app = api_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("x-forwarded-for", next_client_identity())
                .body(Body::from(json.to_string()))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    let status = response.status();
    let bytes = to_bytes(response.into_body())
        .await
        .expect("failed to read response body");
    let body: serde_json::Value =
        serde_json::from_slice(&bytes).expect("response body is not valid json");
    (status, body)
}

async fn get_json(path: &str) -> (StatusCode, serde_json::Value) {
    let _ = ensure_test_data_dir();
    let app = api_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .header("x-forwarded-for", next_client_identity())
                .body(Body::empty())
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    let status = response.status();
    let bytes = to_bytes(response.into_body())
        .await
        .expect("failed to read response body");
    let body: serde_json::Value =
        serde_json::from_slice(&bytes).expect("response body is not valid json");
    (status, body)
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_nanos();
    format!("{}-{}", prefix, nanos)
}

fn write_session_manifest_fixture(
    session_id: &str,
    phase: &str,
    progress_percent: u8,
    resume_required: bool,
    resume_hint: Option<&str>,
) -> String {
    let session_dir = test_data_path(&["wipe_sessions"]);
    let session_path = format!("{}/{}.json", session_dir, session_id);

    fs::create_dir_all(session_dir).expect("failed to create session dir");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "session_id": session_id,
        "created_at": "2026-01-01T00:00:00Z",
        "mode": "offline",
        "target_device_id": "disk1",
        "target_device_model": "model1",
        "target_device_size_gb": 100,
        "target_device_serial": null,
        "method": "overwrite",
        "estimated_minutes": 1,
        "risk_level": "low",
        "final_confirmation_required": "ERASE",
        "phase": phase,
        "progress_percent": progress_percent,
        "resume_required": resume_required,
        "resume_hint": resume_hint
    });
    fs::write(
        &session_path,
        serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
    )
    .expect("failed to write manifest");

    session_path
}

fn write_session_manifest_with_target_fixture(
    session_id: &str,
    phase: &str,
    target_device_id: &str,
    target_device_model: &str,
    target_device_size_gb: u64,
) -> String {
    let session_dir = test_data_path(&["wipe_sessions"]);
    let session_path = format!("{}/{}.json", session_dir, session_id);

    fs::create_dir_all(session_dir).expect("failed to create session dir");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "session_id": session_id,
        "created_at": "2026-01-01T00:00:00Z",
        "mode": "offline",
        "target_device_id": target_device_id,
        "target_device_model": target_device_model,
        "target_device_size_gb": target_device_size_gb,
        "target_device_serial": null,
        "method": "overwrite",
        "estimated_minutes": 1,
        "risk_level": "low",
        "final_confirmation_required": "ERASE",
        "phase": phase,
        "progress_percent": 35,
        "resume_required": true,
        "resume_hint": "awaiting_offline_result_ingest"
    });

    fs::write(
        &session_path,
        serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
    )
    .expect("failed to write manifest");

    session_path
}

fn build_ingest_payload(
    schema_version: u32,
    session_id: &str,
    target_device_id: &str,
    target_device_model: &str,
    target_device_size_gb: u64,
) -> String {
    serde_json::to_string(&serde_json::json!({
        "schema_version": schema_version,
        "session_id": session_id,
        "target_device_id": target_device_id,
        "target_device_model": target_device_model,
        "target_device_size_gb": target_device_size_gb,
        "verification_passed": true,
        "verification_notes": null,
        "completion_status": "verified",
        "verification_evidence": {
            "sample_blocks_checked": 8,
            "sample_blocks_anomalies": 0,
            "checksum_algorithm": "sha256",
            "verification_tool": "integration-test",
            "operator_id": "tester"
        }
    }))
    .expect("failed to serialize request payload")
}

fn write_devices_fixture(path: &str) {
    let devices = serde_json::json!([
        {
            "id": "disk-test-target",
            "dev_type": "USB",
            "model": "FixtureTarget",
            "serial": "SER-TARGET-1",
            "size_gb": 128,
            "allocated_gb": 32,
            "partitions": [],
            "connection": "USB",
            "removable": true,
            "is_system": false,
            "smart_status": "OK",
            "temperature_c": 30.0,
            "encrypted": false,
            "hpa_dco": false,
            "firmware": "FW1",
            "error": null,
            "metadata": {},
            "detection_confidence": {
                "encrypted": "measured",
                "hpa_dco": "measured",
                "is_system": "measured"
            }
        },
        {
            "id": "usb-test-prepare",
            "dev_type": "USB",
            "model": "FixtureUsb",
            "serial": "SER-USB-1",
            "size_gb": 64,
            "allocated_gb": 4,
            "partitions": [],
            "connection": "USB",
            "removable": true,
            "is_system": false,
            "smart_status": "OK",
            "temperature_c": 29.0,
            "encrypted": false,
            "hpa_dco": false,
            "firmware": "FW1",
            "error": null,
            "metadata": {},
            "detection_confidence": {
                "encrypted": "measured",
                "hpa_dco": "measured",
                "is_system": "measured"
            }
        }
    ]);

    fs::write(
        path,
        serde_json::to_string_pretty(&devices).expect("failed to serialize fixture devices"),
    )
    .expect("failed to write fixture devices");
}

#[tokio::test]
async fn integration_wipe_confirm_init_empty_ids_returns_422() {
    let (status, body) = post_json(
        "/api/wipe/confirm-init",
        r#"{"device_ids":[],"method":"overwrite"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("device_ids_required")
    );
}

#[tokio::test]
async fn integration_offline_execute_wrong_confirmation_returns_422() {
    let (status, body) = post_json(
        "/api/offline/wipe/execute",
        r#"{"session_id":"s1","confirmation_text":"NO"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("offline_confirmation_invalid")
    );
}

#[tokio::test]
async fn integration_advisor_empty_device_ids_returns_422() {
    let (status, body) = post_json(
        "/api/advisor/recommend",
        r#"{"device_ids":[],"compliance":"gdpr"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("device_ids_required")
    );
}

#[tokio::test]
async fn integration_create_session_empty_mode_returns_422() {
    let (status, body) = post_json(
        "/api/wipe/session/create",
        r#"{"mode":"","target_device_id":"disk1"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body.get("code").and_then(|v| v.as_str()), Some("mode_required"));
}

#[tokio::test]
async fn integration_prepare_usb_empty_session_id_returns_422() {
    let (status, body) = post_json(
        "/api/usb/prepare",
        r#"{"session_id":"","usb_device_id":"usb1"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("session_id_required")
    );
}

#[tokio::test]
async fn integration_prepare_usb_real_mode_requires_overwrite_confirmation() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    unsafe {
        std::env::set_var("SECUREWIPE_USB_PROVISION_MODE", "real");
        std::env::set_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION", "1");
    }

    let (status, body) = post_json(
        "/api/usb/prepare",
        r#"{"session_id":"any-session","usb_device_id":"usb1"}"#,
    )
    .await;

    unsafe {
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION");
    }

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("usb_overwrite_confirmation_required")
    );
}

#[tokio::test]
async fn integration_prepare_usb_real_mode_requires_breakglass() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    let session_prefix = unique_id("fixture-real-breakglass");
    let fixture_path = test_data_path(&[&format!("{}_devices.json", session_prefix)]);
    write_devices_fixture(&fixture_path);

    unsafe {
        std::env::set_var("SECUREWIPE_DEVICE_FIXTURE_PATH", &fixture_path);
        std::env::set_var("SECUREWIPE_USB_PROVISION_MODE", "real");
        std::env::set_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION", "0");
        std::env::set_var("SECUREWIPE_USB_REAL_ALLOWLIST", "usb-test-prepare");
        std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
    }

    let create_payload = r#"{"mode":"offline","target_device_id":"disk-test-target","compliance":"nist"}"#;
    let (create_status, create_body) = post_json("/api/wipe/session/create", create_payload).await;
    assert_eq!(create_status, StatusCode::OK);

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session id should be returned")
        .to_string();

    let prepare_payload = serde_json::json!({
        "session_id": session_id,
        "usb_device_id": "usb-test-prepare"
    })
    .to_string();
    let (prepare_status, prepare_body) = post_json("/api/usb/prepare", &prepare_payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_DEVICE_FIXTURE_PATH");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION");
        std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
    }

    assert_eq!(prepare_status, StatusCode::FORBIDDEN);
    assert_eq!(
        prepare_body.get("code").and_then(|v| v.as_str()),
        Some("usb_real_breakglass_required")
    );

    let _ = fs::remove_file(test_data_path(&["wipe_sessions", &format!("{}.json", session_id)]));
    let _ = fs::remove_dir_all(test_data_path(&["bootable_usb", &session_id]));
    let _ = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn integration_prepare_usb_real_mode_requires_real_provision_enabled() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    let session_prefix = unique_id("fixture-real-disabled");
    let fixture_path = test_data_path(&[&format!("{}_devices.json", session_prefix)]);
    write_devices_fixture(&fixture_path);

    unsafe {
        std::env::set_var("SECUREWIPE_DEVICE_FIXTURE_PATH", &fixture_path);
        std::env::set_var("SECUREWIPE_USB_PROVISION_MODE", "real");
        std::env::set_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION", "0");
        std::env::set_var("SECUREWIPE_USB_REAL_BREAKGLASS", "1");
        std::env::set_var("SECUREWIPE_USB_REAL_ALLOWLIST", "usb-test-prepare");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
    }

    let create_payload = r#"{"mode":"offline","target_device_id":"disk-test-target","compliance":"nist"}"#;
    let (create_status, create_body) = post_json("/api/wipe/session/create", create_payload).await;
    assert_eq!(create_status, StatusCode::OK);

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session id should be returned")
        .to_string();

    let prepare_payload = serde_json::json!({
        "session_id": session_id,
        "usb_device_id": "usb-test-prepare"
    })
    .to_string();
    let (prepare_status, prepare_body) = post_json("/api/usb/prepare", &prepare_payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_DEVICE_FIXTURE_PATH");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION");
        std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
        std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
    }

    assert_eq!(prepare_status, StatusCode::FORBIDDEN);
    assert_eq!(
        prepare_body.get("code").and_then(|v| v.as_str()),
        Some("usb_real_provisioning_not_enabled")
    );

    let _ = fs::remove_file(test_data_path(&["wipe_sessions", &format!("{}.json", session_id)]));
    let _ = fs::remove_dir_all(test_data_path(&["bootable_usb", &session_id]));
    let _ = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn integration_prepare_usb_real_mode_requires_provision_command() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    let session_prefix = unique_id("fixture-real-missing-cmd");
    let fixture_path = test_data_path(&[&format!("{}_devices.json", session_prefix)]);
    write_devices_fixture(&fixture_path);

    unsafe {
        std::env::set_var("SECUREWIPE_DEVICE_FIXTURE_PATH", &fixture_path);
        std::env::set_var("SECUREWIPE_USB_PROVISION_MODE", "real");
        std::env::set_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION", "0");
        std::env::set_var("SECUREWIPE_USB_REAL_BREAKGLASS", "1");
        std::env::set_var("SECUREWIPE_USB_REAL_ALLOWLIST", "usb-test-prepare");
        std::env::set_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED", "1");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_COMMAND");
    }

    let create_payload = r#"{"mode":"offline","target_device_id":"disk-test-target","compliance":"nist"}"#;
    let (create_status, create_body) = post_json("/api/wipe/session/create", create_payload).await;
    assert_eq!(create_status, StatusCode::OK);

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session id should be returned")
        .to_string();

    let prepare_payload = serde_json::json!({
        "session_id": session_id,
        "usb_device_id": "usb-test-prepare"
    })
    .to_string();
    let (prepare_status, prepare_body) = post_json("/api/usb/prepare", &prepare_payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_DEVICE_FIXTURE_PATH");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION");
        std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
        std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_COMMAND");
    }

    assert_eq!(prepare_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        prepare_body.get("code").and_then(|v| v.as_str()),
        Some("usb_real_provision_command_missing")
    );

    let _ = fs::remove_file(test_data_path(&["wipe_sessions", &format!("{}.json", session_id)]));
    let _ = fs::remove_dir_all(test_data_path(&["bootable_usb", &session_id]));
    let _ = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn integration_prepare_usb_real_mode_harmless_command_smoke() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    let session_prefix = unique_id("fixture-real-smoke");
    let fixture_path = test_data_path(&[&format!("{}_devices.json", session_prefix)]);
    write_devices_fixture(&fixture_path);

    let (command, args_json) = if cfg!(windows) {
        ("cmd", r#"["/C","echo securewipe-usb-smoke"]"#)
    } else {
        ("sh", r#"["-c","echo securewipe-usb-smoke"]"#)
    };

    unsafe {
        std::env::set_var("SECUREWIPE_DEVICE_FIXTURE_PATH", &fixture_path);
        std::env::set_var("SECUREWIPE_USB_PROVISION_MODE", "real");
        std::env::set_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION", "0");
        std::env::set_var("SECUREWIPE_USB_REAL_BREAKGLASS", "1");
        std::env::set_var("SECUREWIPE_USB_REAL_ALLOWLIST", "usb-test-prepare");
        std::env::set_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED", "1");
        std::env::set_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND", command);
        std::env::set_var("SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON", args_json);
    }

    let create_payload = r#"{"mode":"offline","target_device_id":"disk-test-target","compliance":"nist"}"#;
    let (create_status, create_body) = post_json("/api/wipe/session/create", create_payload).await;
    assert_eq!(create_status, StatusCode::OK);

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session id should be returned")
        .to_string();

    let prepare_payload = serde_json::json!({
        "session_id": session_id,
        "usb_device_id": "usb-test-prepare"
    })
    .to_string();
    let (prepare_status, prepare_body) = post_json("/api/usb/prepare", &prepare_payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_DEVICE_FIXTURE_PATH");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION");
        std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
        std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON");
    }

    assert_eq!(prepare_status, StatusCode::OK);
    assert_eq!(
        prepare_body.get("status").and_then(|v| v.as_str()),
        Some("bootable_usb_prepared_real")
    );
    assert_eq!(
        prepare_body.get("bootable_verified").and_then(|v| v.as_bool()),
        Some(true)
    );
    let report_path = prepare_body
        .get("provision_report_path")
        .and_then(|v| v.as_str())
        .expect("provision report path should be returned");
    assert!(
        fs::metadata(report_path).is_ok(),
        "expected report path to exist: {}",
        report_path
    );

    let _ = fs::remove_file(test_data_path(&["wipe_sessions", &format!("{}.json", session_id)]));
    let _ = fs::remove_dir_all(test_data_path(&["bootable_usb", &session_id]));
    let _ = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn integration_session_create_and_usb_prepare_simulation_with_fixture_devices() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    let session_prefix = unique_id("fixture-session");
    let fixture_path = test_data_path(&[&format!("{}_devices.json", session_prefix)]);
    write_devices_fixture(&fixture_path);

    unsafe {
        std::env::set_var("SECUREWIPE_DEVICE_FIXTURE_PATH", &fixture_path);
        std::env::set_var("SECUREWIPE_USB_PROVISION_MODE", "simulation");
        std::env::set_var("SECUREWIPE_DISABLE_OFFLINE_RUNTIME_AUTO_DISCOVERY", "1");
    }

    let create_payload = r#"{"mode":"offline","target_device_id":"disk-test-target","compliance":"nist"}"#;
    let (create_status, create_body) = post_json("/api/wipe/session/create", create_payload).await;
    assert_eq!(create_status, StatusCode::OK);
    assert_eq!(
        create_body.get("status").and_then(|v| v.as_str()),
        Some("session_created")
    );

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session id should be returned")
        .to_string();

    let prepare_payload = serde_json::json!({
        "session_id": session_id,
        "usb_device_id": "usb-test-prepare"
    })
    .to_string();
    let (prepare_status, prepare_body) = post_json("/api/usb/prepare", &prepare_payload).await;
    assert_eq!(prepare_status, StatusCode::OK);
    assert_eq!(
        prepare_body.get("status").and_then(|v| v.as_str()),
        Some("bootable_usb_prepared_simulation")
    );
    assert_eq!(
        prepare_body
            .get("phase")
            .and_then(|v| v.as_str()),
        Some("usb_prepared")
    );

    let (status_code, status_body) =
        get_json(&format!("/api/wipe/session/{}/status", session_id)).await;
    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(
        status_body.get("phase").and_then(|v| v.as_str()),
        Some("usb_prepared")
    );

    unsafe {
        std::env::remove_var("SECUREWIPE_DEVICE_FIXTURE_PATH");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_DISABLE_OFFLINE_RUNTIME_AUTO_DISCOVERY");
    }

    let _ = fs::remove_file(test_data_path(&["wipe_sessions", &format!("{}.json", session_id)]));
    let _ = fs::remove_dir_all(test_data_path(&["bootable_usb", &session_id]));
    let _ = fs::remove_file(&fixture_path);
}

#[tokio::test]
async fn integration_prepare_usb_rejects_non_suitable_device() {
    let _env_guard = env_test_lock().lock().expect("env test lock poisoned");

    unsafe {
        std::env::remove_var("SECUREWIPE_DEVICE_FIXTURE_PATH");
        std::env::remove_var("SECUREWIPE_USB_PROVISION_MODE");
        std::env::remove_var("SECUREWIPE_USB_REQUIRE_OVERWRITE_CONFIRMATION");
        std::env::remove_var("SECUREWIPE_USB_REAL_BREAKGLASS");
        std::env::remove_var("SECUREWIPE_USB_REAL_ALLOWLIST");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ENABLED");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_COMMAND");
        std::env::remove_var("SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON");
    }

    // Write a session fixture in in_app_prepared phase so the handler can reach the device check.
    let session_id = unique_id("integration-usb-suitability");
    write_session_manifest_fixture(
        &session_id,
        "in_app_prepared",
        20,
        true,
        Some("prepare_bootable_usb"),
    );

    // Query real devices so we can find a non-removable system disk to trigger
    // the usb_device_not_removable rejection code.
    let (_, devices_body) = get_json("/api/devices").await;
    let empty = vec![];
    let devices = devices_body.as_array().unwrap_or(&empty);

    let non_removable = devices.iter().find(|d| {
        let removable = d.get("removable").and_then(|v| v.as_bool()).unwrap_or(false);
        let dev_type = d.get("dev_type").and_then(|v| v.as_str()).unwrap_or("");
        !removable && !dev_type.eq_ignore_ascii_case("USB")
    });

    if let Some(device) = non_removable {
        let device_id = device.get("id").and_then(|v| v.as_str()).unwrap_or("disk0");
        let payload = serde_json::json!({
            "session_id": session_id,
            "usb_device_id": device_id,
        })
        .to_string();

        let (status, body) = post_json("/api/usb/prepare", &payload).await;

        assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
            "expected non-suitable or not-found rejection but got {}",
            status
        );
        if status == StatusCode::FORBIDDEN {
            assert_eq!(
                body.get("code").and_then(|v| v.as_str()),
                Some("usb_device_not_removable")
            );
        }
    } else {
        // No non-removable device found (unusual environment): force size failure
        // by setting an impossibly high minimum size.
        unsafe {
            std::env::set_var("SECUREWIPE_USB_MIN_SIZE_GB", "999999");
        }
        let fallback_id = devices
            .first()
            .and_then(|d| d.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("disk0");
        let payload = serde_json::json!({
            "session_id": session_id,
            "usb_device_id": fallback_id,
        })
        .to_string();

        let (status, _body) = post_json("/api/usb/prepare", &payload).await;

        unsafe {
            std::env::remove_var("SECUREWIPE_USB_MIN_SIZE_GB");
        }

        // Either the device was not found or it was too small.
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::NOT_FOUND,
            "expected a suitability rejection but got {}",
            status
        );
    }
}

#[tokio::test]
async fn integration_ingest_result_empty_target_device_id_returns_422() {
    let (status, body) = post_json(
        "/api/offline/result/ingest",
        r#"{"schema_version":1,"session_id":"s1","target_device_id":"","target_device_model":"model1","target_device_size_gb":100,"verification_passed":true,"verification_notes":null,"completion_status":"verified","verification_evidence":{"sample_blocks_checked":8,"sample_blocks_anomalies":0,"checksum_algorithm":"sha256","verification_tool":"integration-test","operator_id":"tester"}}"#,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("target_device_id_required")
    );
}

#[tokio::test]
async fn integration_system_health_endpoint_returns_shape() {
    let (status, body) = get_json("/api/system/health").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.get("health").is_some(),
        "system health response should contain 'health'"
    );
    assert!(
        body.get("update_available").is_some(),
        "system health response should contain 'update_available'"
    );
}

#[tokio::test]
async fn integration_system_security_endpoint_returns_shape() {
    let (status, body) = get_json("/api/system/security").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.get("status").is_some(),
        "system security response should contain 'status'"
    );
    assert!(
        body.get("protections_active").is_some(),
        "system security response should contain 'protections_active'"
    );
}

#[tokio::test]
async fn integration_preflight_endpoint_returns_mvp_shape() {
    let (status, body) = get_json("/api/preflight/mvp").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("host_os").is_some());
    assert!(body.get("mvp_supported").is_some());
    assert!(body.get("notes").and_then(|v| v.as_array()).is_some());
}

#[tokio::test]
async fn integration_chatbot_too_long_message_returns_400() {
    let long_message = "a".repeat(2001);
    let payload = serde_json::to_string(&serde_json::json!({
        "message": long_message,
        "concise": true
    }))
    .expect("failed to serialize chatbot payload");

    let (status, body) = post_json("/api/chatbot", &payload).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("message_too_long")
    );
}

#[tokio::test]
async fn integration_wipe_start_without_final_confirmation_returns_409() {
    let wipe_id = unique_id("integration-confirm");
    let token = unique_id("integration-token");
    let confirmation_dir = test_data_path(&["confirmations"]);
    let confirmation_path = format!("{}/{}.json", confirmation_dir, wipe_id);

    fs::create_dir_all(confirmation_dir).expect("failed to create confirmations dir");
    let state = serde_json::json!({
        "wipe_id": wipe_id,
        "confirmation_token": token,
        "device_ids": [],
        "target_identities": [],
        "method": "overwrite",
        "created_at": "2026-01-01T00:00:00Z",
        "flow_state": "initialized"
    });
    fs::write(
        &confirmation_path,
        serde_json::to_string_pretty(&state).expect("failed to serialize confirmation state"),
    )
    .expect("failed to write confirmation state");

    let payload = serde_json::json!({
        "wipe_id": state["wipe_id"],
        "confirmation_token": state["confirmation_token"]
    });
    let (status, body) = post_json(
        "/api/wipe/start",
        &serde_json::to_string(&payload).expect("failed to serialize request payload"),
    )
    .await;

    let _ = fs::remove_file(&confirmation_path);

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("confirmation_incomplete")
    );
}

#[tokio::test]
async fn integration_wipe_start_legacy_payload_returns_200() {
    let (status, body) = post_json(
        "/api/wipe/start",
        r#"{"device_ids":["disk1"],"method":"auto"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.get("status").and_then(|v| v.as_str()),
        Some("started_simulation_mode")
    );
    assert_eq!(body.get("wipe_id").and_then(|v| v.as_str()).is_some(), true);
}

#[tokio::test]
async fn integration_ingest_result_invalid_phase_returns_409() {
    let session_id = unique_id("integration-phase");
    let session_path =
        write_session_manifest_fixture(&session_id, "in_app_prepared", 10, false, None);

    let payload = build_ingest_payload(1, &session_id, "disk1", "model1", 100);
    let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

    let _ = fs::remove_file(&session_path);

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("invalid_session_phase_for_result_ingest")
    );
}

#[tokio::test]
async fn integration_ingest_result_identity_mismatch_returns_409() {
    let session_id = unique_id("integration-identity");
    let session_path = write_session_manifest_fixture(
        &session_id,
        "wiping",
        70,
        true,
        Some("awaiting_offline_result_ingest"),
    );

    let payload = build_ingest_payload(1, &session_id, "disk2", "model1", 100);
    let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

    let _ = fs::remove_file(&session_path);

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("offline_result_identity_mismatch")
    );
}

#[tokio::test]
async fn integration_ingest_result_unsupported_schema_returns_409() {
    let session_id = unique_id("integration-schema");
    let session_path = write_session_manifest_fixture(
        &session_id,
        "wiping",
        70,
        true,
        Some("awaiting_offline_result_ingest"),
    );

    let payload = build_ingest_payload(999, &session_id, "disk1", "model1", 100);
    let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

    let _ = fs::remove_file(&session_path);

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("unsupported_offline_result_schema_version")
    );
}

#[tokio::test]
async fn integration_ingest_result_anomaly_detector_pauses_operation() {
    let session_id = unique_id("integration-anomaly");
    let session_path = write_session_manifest_fixture(
        &session_id,
        "wiping",
        70,
        true,
        Some("awaiting_offline_result_ingest"),
    );

    let payload = serde_json::to_string(&serde_json::json!({
        "schema_version": 1,
        "session_id": session_id,
        "target_device_id": "disk1",
        "target_device_model": "model1",
        "target_device_size_gb": 100,
        "verification_passed": true,
        "verification_notes": "anomaly seen in post-wipe telemetry",
        "completion_status": "verified",
        "verification_evidence": {
            "sample_blocks_checked": 8,
            "sample_blocks_anomalies": 0,
            "checksum_algorithm": "sha256",
            "verification_tool": "integration-test",
            "operator_id": "tester"
        }
    }))
    .expect("failed to serialize request payload");

    let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body.get("code").and_then(|v| v.as_str()),
        Some("offline_wipe_anomaly_detected")
    );

    let status_path = format!("/api/wipe/session/{}/status", session_id);
    let (status_code, status_body) = get_json(&status_path).await;

    let _ = fs::remove_file(&session_path);

    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(status_body.get("phase").and_then(|v| v.as_str()), Some("failed"));
    assert_eq!(
        status_body.get("resume_hint").and_then(|v| v.as_str()),
        Some("manual_anomaly_review_required")
    );
}

#[tokio::test]
async fn integration_devices_endpoint_returns_array_and_typed_detection_confidence() {
    let (status, body) = get_json("/api/devices").await;

    assert_eq!(status, StatusCode::OK);

    let devices = body
        .as_array()
        .expect("/api/devices response should be a JSON array");

    #[cfg(target_os = "windows")]
    if let Some(first) = devices.first() {
        let confidence = first
            .get("detection_confidence")
            .and_then(|v| v.as_object())
            .expect("device detection_confidence should be an object");

        assert!(
            confidence.contains_key("encrypted"),
            "windows device detection_confidence should include encrypted"
        );
        assert!(
            confidence.contains_key("hpa_dco"),
            "windows device detection_confidence should include hpa_dco"
        );
        assert!(
            confidence.contains_key("is_system"),
            "windows device detection_confidence should include is_system"
        );
    }
}

#[tokio::test]
async fn integration_offline_execute_blocks_when_confidence_unknown_without_override() {
    let (status, body) = get_json("/api/devices").await;
    assert_eq!(status, StatusCode::OK);

    let devices = body
        .as_array()
        .expect("/api/devices response should be an array");
    let candidate = devices.iter().find(|d| {
        let is_system = d.get("is_system").and_then(|v| v.as_bool()).unwrap_or(false);
        let removable = d.get("removable").and_then(|v| v.as_bool()).unwrap_or(false);
        !is_system && removable
    });

    let Some(device) = candidate else {
        return;
    };

    let session_id = unique_id("integration-offline-confidence-block");
    let session_path = write_session_manifest_with_target_fixture(
        &session_id,
        "usb_prepared",
        device.get("id").and_then(|v| v.as_str()).expect("device id missing"),
        device
            .get("model")
            .and_then(|v| v.as_str())
            .expect("device model missing"),
        device
            .get("size_gb")
            .and_then(|v| v.as_u64())
            .expect("device size missing"),
    );

    unsafe {
        std::env::set_var("SECUREWIPE_STRICT_TARGETING", "0");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION");
    }

    let payload = serde_json::to_string(&serde_json::json!({
        "session_id": session_id,
        "confirmation_text": "ERASE"
    }))
    .expect("failed to serialize execute payload");
    let (execute_status, execute_body) = post_json("/api/offline/wipe/execute", &payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_STRICT_TARGETING");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION");
    }
    let _ = fs::remove_file(&session_path);

    assert_eq!(execute_status, StatusCode::FORBIDDEN);
    assert_eq!(
        execute_body.get("code").and_then(|v| v.as_str()),
        Some("detection_confidence_insufficient")
    );
}

#[tokio::test]
async fn integration_offline_execute_override_bypasses_confidence_gate() {
    let (status, body) = get_json("/api/devices").await;
    assert_eq!(status, StatusCode::OK);

    let devices = body
        .as_array()
        .expect("/api/devices response should be an array");
    let candidate = devices.iter().find(|d| {
        let is_system = d.get("is_system").and_then(|v| v.as_bool()).unwrap_or(false);
        let removable = d.get("removable").and_then(|v| v.as_bool()).unwrap_or(false);
        !is_system && removable
    });

    let Some(device) = candidate else {
        return;
    };

    let session_id = unique_id("integration-offline-confidence-override");
    let session_path = write_session_manifest_with_target_fixture(
        &session_id,
        "usb_prepared",
        device.get("id").and_then(|v| v.as_str()).expect("device id missing"),
        device
            .get("model")
            .and_then(|v| v.as_str())
            .expect("device model missing"),
        device
            .get("size_gb")
            .and_then(|v| v.as_u64())
            .expect("device size missing"),
    );

    unsafe {
        std::env::set_var("SECUREWIPE_STRICT_TARGETING", "0");
        std::env::set_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", "1");
    }

    let payload = serde_json::to_string(&serde_json::json!({
        "session_id": session_id,
        "confirmation_text": "ERASE"
    }))
    .expect("failed to serialize execute payload");
    let (execute_status, execute_body) = post_json("/api/offline/wipe/execute", &payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_STRICT_TARGETING");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
    }
    let _ = fs::remove_file(&session_path);

    assert_eq!(execute_status, StatusCode::FORBIDDEN);
    assert_eq!(
        execute_body.get("code").and_then(|v| v.as_str()),
        Some("offline_wipe_blocked")
    );
}

#[tokio::test]
async fn integration_create_session_persists_stage1_snapshot_chain_fields() {
    let (status, body) = get_json("/api/devices").await;
    assert_eq!(status, StatusCode::OK);

    let devices = body
        .as_array()
        .expect("/api/devices response should be an array");
    let candidate = devices.iter().find(|d| {
        let is_system = d.get("is_system").and_then(|v| v.as_bool()).unwrap_or(false);
        let removable = d.get("removable").and_then(|v| v.as_bool()).unwrap_or(false);
        !is_system && removable
    });
    let Some(device) = candidate else {
        return;
    };

    unsafe {
        std::env::set_var("SECUREWIPE_STRICT_TARGETING", "0");
        std::env::set_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", "1");
        std::env::set_var(
            "SECUREWIPE_CERT_SIGNING_SEED",
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );
    }

    let create_payload = serde_json::to_string(&serde_json::json!({
        "mode": "offline",
        "target_device_id": device.get("id").and_then(|v| v.as_str()).expect("device id missing")
    }))
    .expect("failed to serialize create payload");
    let (create_status, create_body) = post_json("/api/wipe/session/create", &create_payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_STRICT_TARGETING");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
        std::env::remove_var("SECUREWIPE_CERT_SIGNING_SEED");
    }

    assert_eq!(create_status, StatusCode::OK);
    let manifest_path = create_body
        .get("manifest_path")
        .and_then(|v| v.as_str())
        .expect("manifest path missing");

    let manifest_raw = fs::read_to_string(manifest_path).expect("failed to read manifest");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("manifest should be valid json");

    assert!(
        manifest_json
            .get("target_detection_snapshot")
            .and_then(|v| v.as_object())
            .is_some(),
        "target_detection_snapshot should be present"
    );
    assert!(
        manifest_json
            .get("target_detection_snapshot_sha256")
            .and_then(|v| v.as_str())
            .is_some(),
        "target_detection_snapshot_sha256 should be present"
    );
    assert!(
        manifest_json
            .get("target_detection_snapshot_signature")
            .and_then(|v| v.as_object())
            .is_some(),
        "target_detection_snapshot_signature should be present when signing seed is configured"
    );

    let _ = fs::remove_file(manifest_path);
}

#[tokio::test]
async fn integration_offline_execute_rejects_tampered_stage1_snapshot_hash() {
    let (status, body) = get_json("/api/devices").await;
    assert_eq!(status, StatusCode::OK);

    let devices = body
        .as_array()
        .expect("/api/devices response should be an array");
    let candidate = devices.iter().find(|d| {
        let is_system = d.get("is_system").and_then(|v| v.as_bool()).unwrap_or(false);
        let removable = d.get("removable").and_then(|v| v.as_bool()).unwrap_or(false);
        !is_system && removable
    });
    let Some(device) = candidate else {
        return;
    };

    unsafe {
        std::env::set_var("SECUREWIPE_STRICT_TARGETING", "0");
        std::env::set_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", "1");
        std::env::set_var(
            "SECUREWIPE_CERT_SIGNING_SEED",
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );
    }

    let create_payload = serde_json::to_string(&serde_json::json!({
        "mode": "offline",
        "target_device_id": device.get("id").and_then(|v| v.as_str()).expect("device id missing")
    }))
    .expect("failed to serialize create payload");
    let (create_status, create_body) = post_json("/api/wipe/session/create", &create_payload).await;
    assert_eq!(create_status, StatusCode::OK);

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session id missing")
        .to_string();
    let manifest_path = create_body
        .get("manifest_path")
        .and_then(|v| v.as_str())
        .expect("manifest path missing");

    let mut manifest_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(manifest_path).expect("failed to read manifest"),
    )
    .expect("manifest should be valid json");
    manifest_json["phase"] = serde_json::json!("usb_prepared");
    manifest_json["resume_required"] = serde_json::json!(true);
    manifest_json["resume_hint"] = serde_json::json!("awaiting_offline_result_ingest");
    manifest_json["target_detection_snapshot_sha256"] = serde_json::json!("deadbeef");
    fs::write(
        manifest_path,
        serde_json::to_string_pretty(&manifest_json).expect("failed to serialize manifest"),
    )
    .expect("failed to write manifest");

    let execute_payload = serde_json::to_string(&serde_json::json!({
        "session_id": session_id,
        "confirmation_text": "ERASE"
    }))
    .expect("failed to serialize execute payload");
    let (execute_status, execute_body) = post_json("/api/offline/wipe/execute", &execute_payload).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_STRICT_TARGETING");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
        std::env::remove_var("SECUREWIPE_CERT_SIGNING_SEED");
    }
    let _ = fs::remove_file(manifest_path);

    assert_eq!(execute_status, StatusCode::CONFLICT);
    assert_eq!(
        execute_body.get("code").and_then(|v| v.as_str()),
        Some("offline_stage1_snapshot_tampered")
    );
}

#[tokio::test]
async fn integration_list_sessions_returns_array() {
    let session_id = unique_id("list-sessions-test");
    let session_path =
        write_session_manifest_fixture(&session_id, "usb_prepared", 30, false, None);

    let (status, body) = get_json("/api/wipe/sessions").await;

    let _ = fs::remove_file(&session_path);

    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array(), "GET /api/wipe/sessions should return a JSON array");
}

#[tokio::test]
async fn integration_list_sessions_includes_created_session() {
    let session_id = unique_id("list-sessions-include");
    let session_path =
        write_session_manifest_fixture(&session_id, "in_app_prepared", 0, false, None);

    let (status, body) = get_json("/api/wipe/sessions").await;

    let _ = fs::remove_file(&session_path);

    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().expect("expected JSON array");
    let found = arr
        .iter()
        .any(|s| s.get("session_id").and_then(|v| v.as_str()) == Some(&session_id));
    assert!(found, "created session should appear in GET /api/wipe/sessions response");
}

#[tokio::test]
async fn integration_offline_session_happy_path_lifecycle() {
    // ------------------------------------------------------------------
    // This test drives the full offline session lifecycle:
    //   create → list → usb prepare → execute → ingest → cert review
    // ------------------------------------------------------------------
    let (dev_status, dev_body) = get_json("/api/devices").await;
    assert_eq!(dev_status, StatusCode::OK);

    let devices = dev_body.as_array().expect("/api/devices should return array");
    let candidate = devices.iter().find(|d| {
        let is_system = d.get("is_system").and_then(|v| v.as_bool()).unwrap_or(false);
        let removable = d.get("removable").and_then(|v| v.as_bool()).unwrap_or(false);
        !is_system && removable
    });

    // Only run if there is a suitable non-system removable device
    let Some(device) = candidate else {
        return;
    };
    let device_id = device
        .get("id")
        .and_then(|v| v.as_str())
        .expect("device id missing");
    let device_model = device
        .get("model")
        .and_then(|v| v.as_str())
        .expect("device model missing");
    let device_size_gb = device
        .get("size_gb")
        .and_then(|v| v.as_u64())
        .expect("device size missing");

    unsafe {
        std::env::set_var("SECUREWIPE_STRICT_TARGETING", "0");
        std::env::set_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", "1");
        std::env::set_var(
            "SECUREWIPE_CERT_SIGNING_SEED",
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );
    }

    // Step 1 — create session
    let create_payload = serde_json::to_string(&serde_json::json!({
        "mode": "offline",
        "target_device_id": device_id
    }))
    .expect("failed to serialize create payload");
    let (create_status, create_body) = post_json("/api/wipe/session/create", &create_payload).await;
    assert_eq!(create_status, StatusCode::OK, "session create should succeed");

    let session_id = create_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session_id missing from create response")
        .to_string();
    let manifest_path = create_body
        .get("manifest_path")
        .and_then(|v| v.as_str())
        .expect("manifest_path missing from create response")
        .to_string();

    // Step 2 — session appears in list
    let (list_status, list_body) = get_json("/api/wipe/sessions").await;
    assert_eq!(list_status, StatusCode::OK);
    let arr = list_body.as_array().expect("sessions should be array");
    assert!(
        arr.iter()
            .any(|s| s.get("session_id").and_then(|v| v.as_str()) == Some(&session_id)),
        "new session should appear in session list"
    );

    // Step 3 — prepare USB
    let usb_payload = serde_json::to_string(&serde_json::json!({
        "session_id": session_id,
        "usb_device_id": "usb0"
    }))
    .expect("failed to serialize usb payload");
    let (usb_status, _) = post_json("/api/usb/prepare", &usb_payload).await;
    assert_eq!(usb_status, StatusCode::OK, "usb prepare should succeed");

    // Step 4 — execute wipe (requires ERASE confirmation)
    let execute_payload = serde_json::to_string(&serde_json::json!({
        "session_id": session_id,
        "confirmation_text": "ERASE"
    }))
    .expect("failed to serialize execute payload");
    let (execute_status, _) = post_json("/api/offline/wipe/execute", &execute_payload).await;
    assert_eq!(execute_status, StatusCode::OK, "offline execute should succeed");

    // Step 5 — ingest result with full verification evidence
    let ingest_payload = build_ingest_payload(1, &session_id, device_id, device_model, device_size_gb);
    let (ingest_status, ingest_body) = post_json("/api/offline/result/ingest", &ingest_payload).await;
    assert_eq!(ingest_status, StatusCode::OK, "result ingest should succeed: {:?}", ingest_body);

    // Step 6 — verify session status reflects completion
    let status_path = format!("/api/wipe/session/{}/status", session_id);
    let (status_code, status_body) = get_json(&status_path).await;
    assert_eq!(status_code, StatusCode::OK);
    let phase = status_body
        .get("phase")
        .and_then(|v| v.as_str())
        .expect("phase missing from status response");
    assert!(
        phase == "completed" || phase == "verified",
        "phase should be completed or verified after ingest, got: {}",
        phase
    );

    // Step 7 — certificate review should indicate eligibility
    let cert_path = format!("/api/certificate/{}/review", session_id);
    let (cert_status, cert_body) = get_json(&cert_path).await;
    assert_eq!(cert_status, StatusCode::OK);
    assert_eq!(
        cert_body
            .get("certificate_eligible")
            .and_then(|v| v.as_bool()),
        Some(true),
        "session should be certificate-eligible after verified ingest"
    );
    assert_eq!(
        cert_body
            .get("signature_verified")
            .and_then(|v| v.as_bool()),
        Some(true),
        "certificate signature should verify for a valid lifecycle"
    );
    assert_eq!(
        cert_body
            .get("completion_status")
            .and_then(|v| v.as_str()),
        Some("verified"),
        "verified ingest should produce verified certificate review status"
    );

    let evidence = cert_body
        .get("verification_evidence")
        .and_then(|v| v.as_object())
        .expect("verified certificate review should include verification_evidence");
    assert_eq!(
        evidence
            .get("sample_blocks_checked")
            .and_then(|v| v.as_u64()),
        Some(8),
        "expected sample_blocks_checked from ingest payload"
    );
    assert_eq!(
        evidence
            .get("sample_blocks_anomalies")
            .and_then(|v| v.as_u64()),
        Some(0),
        "verified result should have zero sample block anomalies"
    );
    assert_eq!(
        evidence
            .get("checksum_algorithm")
            .and_then(|v| v.as_str()),
        Some("sha256"),
        "expected checksum algorithm from ingest payload"
    );
    assert_eq!(
        evidence
            .get("verification_tool")
            .and_then(|v| v.as_str()),
        Some("integration-test"),
        "expected verification tool from ingest payload"
    );

    unsafe {
        std::env::remove_var("SECUREWIPE_STRICT_TARGETING");
        std::env::remove_var("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE");
        std::env::remove_var("SECUREWIPE_CERT_SIGNING_SEED");
    }

    // Cleanup
    let result_path = test_data_path(&["offline_results", &format!("{}.json", session_id)]);
    let _ = fs::remove_file(&manifest_path);
    let _ = fs::remove_file(&result_path);
}

#[tokio::test]
async fn integration_certificate_review_not_eligible_when_verified_missing_evidence() {
    let session_id = unique_id("integration-cert-missing-evidence");
    let session_dir = test_data_path(&["wipe_sessions"]);
    let session_path = format!("{}/{}.json", session_dir, session_id);
    fs::create_dir_all(session_dir).expect("failed to create sessions dir");
    fs::create_dir_all(test_data_path(&["offline_results"])).expect("failed to create result dir");

    let manifest = serde_json::json!({
        "schema_version": 1,
        "session_id": session_id,
        "created_at": "2026-01-01T00:00:00Z",
        "mode": "offline",
        "target_device_id": "disk1",
        "target_device_model": "missing-evidence-model",
        "target_device_size_gb": 100,
        "target_device_serial": null,
        "method": "overwrite",
        "estimated_minutes": 1,
        "risk_level": "low",
        "final_confirmation_required": "ERASE",
        "phase": "completed",
        "progress_percent": 100,
        "resume_required": false,
        "resume_hint": null
    });
    fs::write(
        &session_path,
        serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
    )
    .expect("failed to write session manifest");

    let result_path = test_data_path(&[
        "offline_results",
        &format!("{}.json", manifest["session_id"].as_str().expect("session_id missing")),
    ]);
    let result = serde_json::json!({
        "schema_version": 1,
        "session_id": manifest["session_id"].as_str().expect("session_id missing"),
        "target_device_id": "disk1",
        "target_device_model": "missing-evidence-model",
        "target_device_size_gb": 100,
        "verification_passed": true,
        "verification_notes": null,
        "completion_status": "verified",
        "ingested_at": "2026-01-01T00:10:00Z"
    });
    fs::write(
        &result_path,
        serde_json::to_string_pretty(&result).expect("failed to serialize result"),
    )
    .expect("failed to write offline result");

    unsafe {
        std::env::set_var(
            "SECUREWIPE_CERT_SIGNING_SEED",
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );
    }

    let review_path = format!(
        "/api/certificate/{}/review",
        manifest["session_id"].as_str().expect("session_id missing")
    );
    let (status, body) = get_json(&review_path).await;

    unsafe {
        std::env::remove_var("SECUREWIPE_CERT_SIGNING_SEED");
    }
    let _ = fs::remove_file(&session_path);
    let _ = fs::remove_file(&result_path);

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.get("certificate_eligible").and_then(|v| v.as_bool()),
        Some(false),
        "missing structured verification evidence should block eligibility"
    );
    assert_eq!(
        body.get("status").and_then(|v| v.as_str()),
        Some("certificate_review_attention_required")
    );
    assert_eq!(
        body.get("issues")
            .and_then(|v| v.as_array())
            .map(|issues| {
                issues.iter().any(|issue| {
                    issue
                        .as_str()
                        .map(|text| text.contains("no structured verification evidence"))
                        .unwrap_or(false)
                })
            }),
        Some(true),
        "review issues should include the missing verification evidence warning"
    );
}
