use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use hyper::body::to_bytes;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

use super::api_router;
use super::storage::read_offline_result;

    async fn post_json(path: &str, json: &str) -> (StatusCode, serde_json::Value) {
        let app = api_router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
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

    async fn post_empty(path: &str) -> (StatusCode, serde_json::Value) {
        let app = api_router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
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

    async fn retry_post_json_non_429(path: &str, json: &str) -> (StatusCode, serde_json::Value) {
        let mut status = StatusCode::TOO_MANY_REQUESTS;
        let mut body = serde_json::json!({});
        for _ in 0..3 {
            reset_guard_limiter();
            let (attempt_status, attempt_body) = post_json(path, json).await;
            status = attempt_status;
            body = attempt_body;
            if status != StatusCode::TOO_MANY_REQUESTS {
                break;
            }
        }
        (status, body)
    }

    async fn post_body(path: &str, body: String, content_length: usize) -> (StatusCode, serde_json::Value) {
        let app = api_router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .header("content-length", content_length.to_string())
                    .body(Body::from(body))
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
        let app = api_router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
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

    async fn retry_get_json_non_429(path: &str) -> (StatusCode, serde_json::Value) {
        let mut status = StatusCode::TOO_MANY_REQUESTS;
        let mut body = serde_json::json!({});
        for _ in 0..3 {
            reset_guard_limiter();
            let (attempt_status, attempt_body) = get_json(path).await;
            status = attempt_status;
            body = attempt_body;
            if status != StatusCode::TOO_MANY_REQUESTS {
                break;
            }
        }
        (status, body)
    }

    async fn get_raw(path: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let app = api_router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("request failed");

        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body())
            .await
            .expect("failed to read response body")
            .to_vec();
        (status, headers, bytes)
    }

    fn unique_id(prefix: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos();
        format!("{}-{}", prefix, nanos)
    }

    fn reset_guard_limiter() {
        super::guards::reset_high_risk_limiter();
    }

    fn set_test_certificate_seed() {
        unsafe {
            std::env::set_var(
                "SECUREWIPE_CERT_SIGNING_SEED",
                "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
            );
        }
    }

    fn write_session_manifest_fixture(session_id: &str, manifest: &serde_json::Value) -> String {
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::write(
            &session_path,
            serde_json::to_string_pretty(manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");
        session_path
    }

    #[tokio::test]
    async fn router_chatbot_empty_message_returns_400_with_code() {
        reset_guard_limiter();
        let (status, body) = post_json("/api/chatbot", r#"{"message":"   "}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.get("code").and_then(|v| v.as_str()), Some("message_required"));
    }

    #[tokio::test]
    async fn router_offline_execute_wrong_confirmation_returns_422_with_code() {
        reset_guard_limiter();
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
    async fn router_wipe_confirm_init_empty_ids_returns_422_with_code() {
        reset_guard_limiter();
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
    async fn router_wipe_confirm_risk_missing_state_returns_404_with_code() {
        reset_guard_limiter();
        let wipe_id = unique_id("missing-risk");
        let payload = format!(
            r#"{{"wipe_id":"{}","confirmation_token":"t1","acknowledged":true}}"#,
            wipe_id
        );
        let (status, body) = post_json("/api/wipe/confirm-risk", &payload).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("confirmation_not_found")
        );
    }

    #[tokio::test]
    async fn router_wipe_start_without_final_confirmation_returns_409() {
        reset_guard_limiter();
        let wipe_id = unique_id("state-init");
        let token = unique_id("token");
        let confirm_dir = "data/confirmations";
        let confirm_path = format!("{}/{}.json", confirm_dir, wipe_id);

        fs::create_dir_all(confirm_dir).expect("failed to create confirmations dir");
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
            &confirm_path,
            serde_json::to_string_pretty(&state).expect("failed to serialize state"),
        )
        .expect("failed to write confirmation state");

        let payload = format!(
            r#"{{"wipe_id":"{}","confirmation_token":"{}"}}"#,
            state["wipe_id"].as_str().expect("wipe_id missing"),
            state["confirmation_token"].as_str().expect("token missing")
        );
        let (status, body) = post_json("/api/wipe/start", &payload).await;

        let _ = fs::remove_file(&confirm_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("confirmation_incomplete")
        );
    }

    #[tokio::test]
    async fn router_high_risk_guard_rejects_large_payload() {
        reset_guard_limiter();
        let huge = "x".repeat(300 * 1024);
        let body = format!(r#"{{"device_ids":[],"method":"{}"}}"#, huge);
        let (status, body) = post_body("/api/wipe/confirm-init", body.clone(), body.len()).await;

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("payload_too_large")
        );
    }

    #[tokio::test]
    async fn router_high_risk_guard_rate_limits_burst_requests() {
        let mut saw_rate_limit = false;
        for _ in 0..8 {
            reset_guard_limiter();
            for _ in 0..50 {
                let (status, body) = post_json(
                    "/api/wipe/confirm-init",
                    r#"{"device_ids":[],"method":"overwrite"}"#,
                )
                .await;
                if status == StatusCode::TOO_MANY_REQUESTS
                    && body.get("code").and_then(|v| v.as_str()) == Some("rate_limited")
                {
                    saw_rate_limit = true;
                    break;
                }
            }
            if saw_rate_limit {
                break;
            }
        }

        assert!(saw_rate_limit, "expected at least one rate_limited response");
        reset_guard_limiter();
    }

    #[tokio::test]
    async fn router_confirmation_flow_happy_path_completes() {
        reset_guard_limiter();

        let wipe_id = unique_id("flow-happy");
        let token = unique_id("flow-token");
        let confirm_dir = "data/confirmations";
        let confirm_path = format!("{}/{}.json", confirm_dir, wipe_id);

        fs::create_dir_all(confirm_dir).expect("failed to create confirmations dir");
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
            &confirm_path,
            serde_json::to_string_pretty(&state).expect("failed to serialize state"),
        )
        .expect("failed to write confirmation state");

        let risk_payload = format!(
            r#"{{"wipe_id":"{}","confirmation_token":"{}","acknowledged":true}}"#,
            state["wipe_id"].as_str().expect("wipe_id missing"),
            state["confirmation_token"].as_str().expect("token missing")
        );
        let (risk_status, risk_body) = retry_post_json_non_429("/api/wipe/confirm-risk", &risk_payload).await;
        assert_eq!(risk_status, StatusCode::OK);
        assert_eq!(
            risk_body.get("status").and_then(|v| v.as_str()),
            Some("risk_acknowledged")
        );

        let final_payload = format!(
            r#"{{"wipe_id":"{}","confirmation_token":"{}","confirmation_text":"ERASE"}}"#,
            state["wipe_id"].as_str().expect("wipe_id missing"),
            state["confirmation_token"].as_str().expect("token missing")
        );
        let (final_status, final_body) = retry_post_json_non_429("/api/wipe/confirm-final", &final_payload).await;
        assert_eq!(final_status, StatusCode::OK);
        assert_eq!(
            final_body.get("status").and_then(|v| v.as_str()),
            Some("final_confirmation_accepted")
        );

        let start_payload = format!(
            r#"{{"wipe_id":"{}","confirmation_token":"{}"}}"#,
            state["wipe_id"].as_str().expect("wipe_id missing"),
            state["confirmation_token"].as_str().expect("token missing")
        );
        let (start_status, start_body) = retry_post_json_non_429("/api/wipe/start", &start_payload).await;

        let _ = fs::remove_file(&confirm_path);

        assert_eq!(start_status, StatusCode::OK);
        assert_eq!(
            start_body.get("status").and_then(|v| v.as_str()),
            Some("started_simulation_mode")
        );
    }

    #[tokio::test]
    async fn router_offline_ingest_updates_phase_and_status_endpoint_reports_it() {
        reset_guard_limiter();

        let session_id = unique_id("session-phase");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);

        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        let manifest = serde_json::json!({
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
            "phase": "wiping",
            "progress_percent": 70
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let ingest_payload = format!(
            r#"{{"schema_version":1,"session_id":"{}","target_device_id":"disk1","target_device_model":"model1","target_device_size_gb":100,"verification_passed":true,"verification_notes":null,"completion_status":"ok","verification_evidence":{{"sample_blocks_checked":8,"sample_blocks_anomalies":0,"checksum_algorithm":"sha256","verification_tool":"router-test","operator_id":"tester"}}}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (ingest_status, ingest_body) =
            post_json("/api/offline/result/ingest", &ingest_payload).await;
        assert_eq!(ingest_status, StatusCode::OK);
        assert_eq!(
            ingest_body.get("phase").and_then(|v| v.as_str()),
            Some("completed")
        );
        assert_eq!(
            ingest_body.get("progress_percent").and_then(|v| v.as_u64()),
            Some(100)
        );
        assert_eq!(
            ingest_body.get("resume_required").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(ingest_body.get("resume_hint"), Some(&serde_json::Value::Null));

        let status_path = format!(
            "/api/wipe/session/{}/status",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status_code, status_body) = get_json(&status_path).await;
        let stored_result = read_offline_result(
            manifest["session_id"].as_str().expect("session_id missing"),
        )
        .expect("expected stored offline result record");

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(format!(
            "data/offline_results/{}.json",
            manifest["session_id"].as_str().expect("session_id missing")
        ));

        assert_eq!(status_code, StatusCode::OK);
        assert_eq!(
            status_body.get("phase").and_then(|v| v.as_str()),
            Some("completed")
        );
        assert_eq!(
            status_body.get("progress_percent").and_then(|v| v.as_u64()),
            Some(100)
        );
        assert_eq!(
            status_body.get("resume_required").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(status_body.get("resume_hint"), Some(&serde_json::Value::Null));
        assert_eq!(stored_result.schema_version, 1);
        assert_eq!(
            stored_result.session_id,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        assert_eq!(stored_result.target_device_id, "disk1");
        assert!(stored_result.verification_passed);
    }

    #[tokio::test]
    async fn router_session_status_reports_phase_progress_pairs() {
        reset_guard_limiter();

        let session_id = unique_id("session-progress");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

        let cases = vec![
            ("in_app_prepared", 10_u64),
            ("usb_prepared", 25_u64),
            ("offline_started", 40_u64),
            ("wiping", 70_u64),
            ("verified", 90_u64),
            ("completed", 100_u64),
        ];

        for (phase, progress) in cases {
            let manifest = serde_json::json!({
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
                "progress_percent": progress
            });

            fs::write(
                &session_path,
                serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
            )
            .expect("failed to write session manifest");

            let status_path = format!(
                "/api/wipe/session/{}/status",
                manifest["session_id"].as_str().expect("session_id missing")
            );
            let mut status_code = StatusCode::TOO_MANY_REQUESTS;
            let mut status_body = serde_json::json!({});
            for _ in 0..3 {
                reset_guard_limiter();
                let (attempt_status, attempt_body) = get_json(&status_path).await;
                status_code = attempt_status;
                status_body = attempt_body;
                if status_code != StatusCode::TOO_MANY_REQUESTS {
                    break;
                }
            }

            assert_eq!(status_code, StatusCode::OK);
            assert_eq!(status_body.get("phase").and_then(|v| v.as_str()), Some(phase));
            assert_eq!(
                status_body.get("progress_percent").and_then(|v| v.as_u64()),
                Some(progress)
            );
        }

        let _ = fs::remove_file(&session_path);
    }

    #[tokio::test]
    async fn router_usb_prepare_rejects_invalid_session_phase() {
        reset_guard_limiter();

        let session_id = unique_id("session-usb-invalid");
        let manifest = serde_json::json!({
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
            "phase": "wiping",
            "progress_percent": 70
        });
        let session_path = write_session_manifest_fixture(
            manifest["session_id"].as_str().expect("session_id missing"),
            &manifest,
        );

        let payload = format!(
            r#"{{"session_id":"{}","usb_device_id":"usb-test"}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_json("/api/usb/prepare", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("invalid_session_phase_for_usb_prepare")
        );
    }

    #[tokio::test]
    async fn router_offline_execute_rejects_invalid_session_phase() {
        reset_guard_limiter();

        let session_id = unique_id("session-exec-invalid");
        let manifest = serde_json::json!({
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
            "phase": "completed",
            "progress_percent": 100
        });
        let session_path = write_session_manifest_fixture(
            manifest["session_id"].as_str().expect("session_id missing"),
            &manifest,
        );

        let payload = format!(
            r#"{{"session_id":"{}","confirmation_text":"ERASE"}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = retry_post_json_non_429("/api/offline/wipe/execute", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("invalid_session_phase_for_offline_start")
        );
    }

    #[tokio::test]
    async fn router_offline_ingest_rejects_invalid_session_phase() {
        reset_guard_limiter();

        let session_id = unique_id("session-ingest-invalid");
        let manifest = serde_json::json!({
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
            "phase": "usb_prepared",
            "progress_percent": 25,
            "resume_required": true,
            "resume_hint": "boot_into_offline_environment"
        });
        let session_path = write_session_manifest_fixture(
            manifest["session_id"].as_str().expect("session_id missing"),
            &manifest,
        );

        let payload = format!(
            r#"{{"schema_version":1,"session_id":"{}","target_device_id":"disk1","target_device_model":"model1","target_device_size_gb":100,"verification_passed":true,"verification_notes":null,"completion_status":"ok","verification_evidence":{{"sample_blocks_checked":8,"sample_blocks_anomalies":0,"checksum_algorithm":"sha256","verification_tool":"router-test","operator_id":"tester"}}}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("invalid_session_phase_for_result_ingest")
        );
    }

    #[tokio::test]
    async fn router_offline_ingest_rejects_identity_mismatch() {
        reset_guard_limiter();

        let session_id = unique_id("session-identity-mismatch");
        let manifest = serde_json::json!({
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
            "phase": "wiping",
            "progress_percent": 70
        });
        let session_path = write_session_manifest_fixture(
            manifest["session_id"].as_str().expect("session_id missing"),
            &manifest,
        );

        let payload = format!(
            r#"{{"schema_version":1,"session_id":"{}","target_device_id":"disk2","target_device_model":"model1","target_device_size_gb":100,"verification_passed":true,"verification_notes":null,"completion_status":"ok","verification_evidence":{{"sample_blocks_checked":8,"sample_blocks_anomalies":0,"checksum_algorithm":"sha256","verification_tool":"router-test","operator_id":"tester"}}}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("offline_result_identity_mismatch")
        );
    }

    #[tokio::test]
    async fn router_usb_prepare_rejects_missing_usb_device() {
        reset_guard_limiter();

        let session_id = unique_id("session-usb-missing");
        let manifest = serde_json::json!({
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
            "phase": "in_app_prepared",
            "progress_percent": 10
        });
        let session_path = write_session_manifest_fixture(
            manifest["session_id"].as_str().expect("session_id missing"),
            &manifest,
        );

        let payload = format!(
            r#"{{"session_id":"{}","usb_device_id":"{}"}}"#,
            manifest["session_id"].as_str().expect("session_id missing"),
            unique_id("missing-usb")
        );
        let (status, body) = post_json("/api/usb/prepare", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("usb_device_not_found")
        );
    }

    #[tokio::test]
    async fn router_session_status_persists_across_router_reinstantiation() {
        reset_guard_limiter();

        let session_id = unique_id("session-recovery");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

        let manifest = serde_json::json!({
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
            "phase": "usb_prepared",
            "progress_percent": 25,
            "resume_required": true,
            "resume_hint": "boot_into_offline_environment"
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let status_path = format!(
            "/api/wipe/session/{}/status",
            manifest["session_id"].as_str().expect("session_id missing")
        );

        let (status_code_1, status_body_1) = retry_get_json_non_429(&status_path).await;
        assert_eq!(status_code_1, StatusCode::OK);
        assert_eq!(status_body_1.get("phase").and_then(|v| v.as_str()), Some("usb_prepared"));
        assert_eq!(
            status_body_1.get("progress_percent").and_then(|v| v.as_u64()),
            Some(25)
        );
        assert_eq!(
            status_body_1.get("resume_required").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            status_body_1.get("resume_hint").and_then(|v| v.as_str()),
            Some("boot_into_offline_environment")
        );

        let (status_code_2, status_body_2) = retry_get_json_non_429(&status_path).await;
        assert_eq!(status_code_2, StatusCode::OK);
        assert_eq!(status_body_2.get("phase").and_then(|v| v.as_str()), Some("usb_prepared"));
        assert_eq!(
            status_body_2.get("progress_percent").and_then(|v| v.as_u64()),
            Some(25)
        );
        assert_eq!(
            status_body_2.get("resume_required").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            status_body_2.get("resume_hint").and_then(|v| v.as_str()),
            Some("boot_into_offline_environment")
        );

        let _ = fs::remove_file(&session_path);
    }

    #[tokio::test]
    async fn router_resume_advances_usb_prepared_to_reboot_pending() {
        reset_guard_limiter();

        let session_id = unique_id("session-resume-advance");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

        let manifest = serde_json::json!({
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
            "phase": "usb_prepared",
            "progress_percent": 25,
            "resume_required": true,
            "resume_hint": "boot_into_offline_environment"
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let resume_path = format!(
            "/api/wipe/session/{}/resume",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let mut status = StatusCode::TOO_MANY_REQUESTS;
        let mut body = serde_json::json!({});
        for _ in 0..3 {
            reset_guard_limiter();
            let (attempt_status, attempt_body) = post_empty(&resume_path).await;
            status = attempt_status;
            body = attempt_body;
            if status != StatusCode::TOO_MANY_REQUESTS {
                break;
            }
        }

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.get("phase").and_then(|v| v.as_str()), Some("reboot_pending"));
        assert_eq!(
            body.get("recommended_action").and_then(|v| v.as_str()),
            Some("reboot_to_offline")
        );
        assert_eq!(body.get("resume_required").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            body.get("resume_hint").and_then(|v| v.as_str()),
            Some("boot_into_offline_environment")
        );

        let status_path = format!(
            "/api/wipe/session/{}/status",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status_code, status_body) = get_json(&status_path).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status_code, StatusCode::OK);
        assert_eq!(status_body.get("phase").and_then(|v| v.as_str()), Some("reboot_pending"));
    }

    #[tokio::test]
    async fn router_resume_rejects_failed_session() {
        reset_guard_limiter();

        let session_id = unique_id("session-resume-failed");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

        let manifest = serde_json::json!({
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
            "phase": "failed",
            "progress_percent": 100,
            "resume_required": false,
            "resume_hint": null
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let resume_path = format!(
            "/api/wipe/session/{}/resume",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_empty(&resume_path).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("session_resume_blocked_failed")
        );
    }

    #[tokio::test]
    async fn router_session_artifacts_returns_validated_typed_summary() {
        reset_guard_limiter();

        let session_id = unique_id("session-artifacts");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");

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

        let result_path = format!("data/offline_results/{}.json", manifest["session_id"].as_str().expect("session_id missing"));
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk1",
            "target_device_model": "model1",
            "target_device_size_gb": 100,
            "verification_passed": true,
            "verification_notes": null,
            "completion_status": "ok",
            "ingested_at": "2026-01-01T00:10:00Z"
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result).expect("failed to serialize result"),
        )
        .expect("failed to write offline result");

        let artifacts_path = format!(
            "/api/wipe/session/{}/artifacts",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = get_json(&artifacts_path).await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.get("status").and_then(|v| v.as_str()), Some("session_artifacts_ready"));
        assert_eq!(body.get("artifact_consistent").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(body.get("manifest_phase").and_then(|v| v.as_str()), Some("completed"));
        assert_eq!(body.get("verification_passed").and_then(|v| v.as_bool()), Some(true));
    }

    #[tokio::test]
    async fn router_session_artifacts_rejects_identity_mismatch() {
        reset_guard_limiter();

        let session_id = unique_id("session-artifacts-mismatch");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");

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

        let result_path = format!("data/offline_results/{}.json", manifest["session_id"].as_str().expect("session_id missing"));
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk2",
            "target_device_model": "model1",
            "target_device_size_gb": 100,
            "verification_passed": true,
            "verification_notes": null,
            "completion_status": "ok",
            "ingested_at": "2026-01-01T00:10:00Z"
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result).expect("failed to serialize result"),
        )
        .expect("failed to write offline result");

        let artifacts_path = format!(
            "/api/wipe/session/{}/artifacts",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = get_json(&artifacts_path).await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("session_artifact_identity_mismatch")
        );
    }

    #[tokio::test]
    async fn router_certificate_prefers_validated_artifacts_over_logs() {
        reset_guard_limiter();
        set_test_certificate_seed();

        let session_id = unique_id("session-certificate");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");
        fs::create_dir_all("data").expect("failed to create data dir");

        let manifest = serde_json::json!({
            "schema_version": 1,
            "session_id": session_id,
            "created_at": "2026-01-01T00:00:00Z",
            "mode": "offline",
            "target_device_id": "disk1",
            "target_device_model": "typed-model",
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

        let result_path = format!(
            "data/offline_results/{}.json",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk1",
            "target_device_model": "typed-model",
            "target_device_size_gb": 100,
            "verification_passed": true,
            "verification_notes": null,
            "completion_status": "ok",
            "ingested_at": "2026-01-01T00:10:00Z"
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result).expect("failed to serialize result"),
        )
        .expect("failed to write offline result");

        let history_path = "data/feedback_history.json";
        let history = serde_json::json!([
            {
                "device_id": "disk1",
                "model": "log-model",
                "recommendation": "legacy-method",
                "explanation": "Legacy history entry",
                "timestamp": "2026-01-01T00:05:00Z",
                "operation_id": "op-1",
                "wipe_id": manifest["session_id"].as_str().expect("session_id missing"),
                "phase": "offline_result_ingested"
            }
        ]);
        fs::write(
            history_path,
            serde_json::to_string_pretty(&history).expect("failed to serialize history"),
        )
        .expect("failed to write history");

        let cert_path = format!(
            "/api/certificate/{}",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = get_json(&cert_path).await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body.get("certificate")
                .and_then(|c| c.get("mode"))
                .and_then(|v| v.as_str()),
            Some("offline")
        );
        assert_eq!(
            body.get("certificate")
                .and_then(|c| c.get("method"))
                .and_then(|v| v.as_str()),
            Some("overwrite")
        );
        assert_eq!(
            body.get("certificate")
                .and_then(|c| c.get("devices"))
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .and_then(|d| d.get("model"))
                .and_then(|v| v.as_str()),
            Some("typed-model")
        );
        assert_eq!(
            body.get("certificate")
                .and_then(|c| c.get("recovery_risk"))
                .and_then(|v| v.as_str()),
            Some("Low (validated typed artifacts)")
        );
        assert_eq!(
            body.get("signature")
                .and_then(|s| s.get("algorithm"))
                .and_then(|v| v.as_str()),
            Some("ed25519")
        );
        assert_eq!(
            body.get("signature")
                .and_then(|s| s.get("public_key_base64"))
                .and_then(|v| v.as_str())
                .map(|v| !v.is_empty()),
            Some(true)
        );
        assert_eq!(
            body.get("signature")
                .and_then(|s| s.get("signature_base64"))
                .and_then(|v| v.as_str())
                .map(|v| !v.is_empty()),
            Some(true)
        );
        assert_eq!(
            body.get("signature_sha256").and_then(|v| v.as_str()),
            body.get("signature")
                .and_then(|s| s.get("payload_sha256"))
                .and_then(|v| v.as_str())
        );
    }

    #[tokio::test]
    async fn router_certificate_verify_accepts_server_signed_certificate() {
        reset_guard_limiter();
        set_test_certificate_seed();

        let session_id = unique_id("session-certificate-verify");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");

        let manifest = serde_json::json!({
            "schema_version": 1,
            "session_id": session_id,
            "created_at": "2026-01-01T00:00:00Z",
            "mode": "offline",
            "target_device_id": "disk1",
            "target_device_model": "verify-model",
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

        let result_path = format!(
            "data/offline_results/{}.json",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk1",
            "target_device_model": "verify-model",
            "target_device_size_gb": 100,
            "verification_passed": true,
            "verification_notes": null,
            "completion_status": "ok",
            "ingested_at": "2026-01-01T00:10:00Z"
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result).expect("failed to serialize result"),
        )
        .expect("failed to write offline result");

        let cert_path = format!(
            "/api/certificate/{}",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (cert_status, cert_body) = get_json(&cert_path).await;

        let verify_payload = serde_json::json!({
            "certificate": cert_body.get("certificate").cloned().expect("certificate missing"),
            "public_key_base64": cert_body
                .get("signature")
                .and_then(|s| s.get("public_key_base64"))
                .cloned()
                .expect("public key missing"),
            "signature_base64": cert_body
                .get("signature")
                .and_then(|s| s.get("signature_base64"))
                .cloned()
                .expect("signature missing")
        });

        let (verify_status, verify_body) = post_json(
            "/api/certificate/verify",
            &serde_json::to_string(&verify_payload).expect("failed to serialize verify payload"),
        )
        .await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(cert_status, StatusCode::OK);
        assert_eq!(verify_status, StatusCode::OK);
        assert_eq!(
            verify_body.get("status").and_then(|v| v.as_str()),
            Some("certificate_verified")
        );
        assert_eq!(
            verify_body.get("verified").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            verify_body.get("payload_sha256").and_then(|v| v.as_str()),
            cert_body
                .get("signature")
                .and_then(|s| s.get("payload_sha256"))
                .and_then(|v| v.as_str())
        );
    }

    #[tokio::test]
    async fn router_certificate_verify_detects_tampered_payload() {
        reset_guard_limiter();
        set_test_certificate_seed();

        let session_id = unique_id("session-certificate-tampered");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");

        let manifest = serde_json::json!({
            "schema_version": 1,
            "session_id": session_id,
            "created_at": "2026-01-01T00:00:00Z",
            "mode": "offline",
            "target_device_id": "disk1",
            "target_device_model": "tamper-model",
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

        let result_path = format!(
            "data/offline_results/{}.json",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk1",
            "target_device_model": "tamper-model",
            "target_device_size_gb": 100,
            "verification_passed": true,
            "verification_notes": null,
            "completion_status": "ok",
            "ingested_at": "2026-01-01T00:10:00Z"
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result).expect("failed to serialize result"),
        )
        .expect("failed to write offline result");

        let cert_path = format!(
            "/api/certificate/{}",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (cert_status, cert_body) = get_json(&cert_path).await;

        let mut tampered_certificate = cert_body
            .get("certificate")
            .cloned()
            .expect("certificate missing");
        tampered_certificate["method"] = serde_json::json!("tampered-method");

        let verify_payload = serde_json::json!({
            "certificate": tampered_certificate,
            "public_key_base64": cert_body
                .get("signature")
                .and_then(|s| s.get("public_key_base64"))
                .cloned()
                .expect("public key missing"),
            "signature_base64": cert_body
                .get("signature")
                .and_then(|s| s.get("signature_base64"))
                .cloned()
                .expect("signature missing")
        });

        let (verify_status, verify_body) = post_json(
            "/api/certificate/verify",
            &serde_json::to_string(&verify_payload).expect("failed to serialize verify payload"),
        )
        .await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(cert_status, StatusCode::OK);
        assert_eq!(verify_status, StatusCode::OK);
        assert_eq!(
            verify_body.get("status").and_then(|v| v.as_str()),
            Some("certificate_verification_failed")
        );
        assert_eq!(
            verify_body.get("verified").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[tokio::test]
    async fn router_certificate_review_reports_eligible_verified_certificate() {
        reset_guard_limiter();
        set_test_certificate_seed();

        let session_id = unique_id("session-certificate-review");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");

        let manifest = serde_json::json!({
            "schema_version": 1,
            "session_id": session_id,
            "created_at": "2026-01-01T00:00:00Z",
            "mode": "offline",
            "target_device_id": "disk1",
            "target_device_model": "review-model",
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

        let result_path = format!(
            "data/offline_results/{}.json",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk1",
            "target_device_model": "review-model",
            "target_device_size_gb": 100,
            "verification_passed": true,
            "verification_notes": null,
            "completion_status": "verified",
            "ingested_at": "2026-01-01T00:10:00Z",
            "verification_evidence": {
                "sample_blocks_checked": 8,
                "sample_blocks_anomalies": 0,
                "checksum_algorithm": "sha256",
                "verification_tool": "router-test",
                "operator_id": "tester"
            }
        });
        fs::write(
            &result_path,
            serde_json::to_string_pretty(&result).expect("failed to serialize result"),
        )
        .expect("failed to write offline result");

        let review_path = format!(
            "/api/certificate/{}/review",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = get_json(&review_path).await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body.get("status").and_then(|v| v.as_str()),
            Some("certificate_review_ready")
        );
        assert_eq!(
            body.get("certificate_eligible").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            body.get("signature_verified").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            body.get("completion_status").and_then(|v| v.as_str()),
            Some("verified")
        );
        assert_eq!(
            body.get("issues")
                .and_then(|v| v.as_array())
                .map(|issues| issues.is_empty()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn router_certificate_pdf_returns_pdf_attachment() {
        reset_guard_limiter();
        set_test_certificate_seed();

        let session_id = unique_id("session-certificate-pdf");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");
        fs::create_dir_all("data/offline_results").expect("failed to create result dir");

        let manifest = serde_json::json!({
            "schema_version": 1,
            "session_id": session_id,
            "created_at": "2026-01-01T00:00:00Z",
            "mode": "offline",
            "target_device_id": "disk1",
            "target_device_model": "pdf-model",
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

        let result_path = format!(
            "data/offline_results/{}.json",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let result = serde_json::json!({
            "schema_version": 1,
            "session_id": manifest["session_id"].as_str().expect("session_id missing"),
            "target_device_id": "disk1",
            "target_device_model": "pdf-model",
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

        let pdf_path = format!(
            "/api/certificate/{}/pdf",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, headers, bytes) = get_raw(&pdf_path).await;

        let _ = fs::remove_file(&session_path);
        let _ = fs::remove_file(&result_path);

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/pdf")
        );
        assert_eq!(
            headers
                .get(header::CONTENT_DISPOSITION)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("attachment; filename=\"securewipe-certificate-")),
            Some(true)
        );
        assert_eq!(bytes.starts_with(b"%PDF-1.4"), true);
        assert_eq!(String::from_utf8_lossy(&bytes).contains("SecureWipe Certificate"), true);
    }

    #[tokio::test]
    async fn router_resume_rejects_unsupported_session_schema_version() {
        reset_guard_limiter();

        let session_id = unique_id("session-resume-schema");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

        let manifest = serde_json::json!({
            "schema_version": 999,
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
            "phase": "usb_prepared",
            "progress_percent": 25,
            "resume_required": true,
            "resume_hint": "boot_into_offline_environment"
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let resume_path = format!(
            "/api/wipe/session/{}/resume",
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_empty(&resume_path).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("unsupported_session_schema_version")
        );
    }

    #[tokio::test]
    async fn router_offline_ingest_rejects_unsupported_session_schema_version() {
        reset_guard_limiter();

        let session_id = unique_id("session-ingest-schema");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

        let manifest = serde_json::json!({
            "schema_version": 999,
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
            "phase": "wiping",
            "progress_percent": 70,
            "resume_required": true,
            "resume_hint": "awaiting_offline_result_ingest"
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let payload = format!(
            r#"{{"schema_version":1,"session_id":"{}","target_device_id":"disk1","target_device_model":"model1","target_device_size_gb":100,"verification_passed":true,"verification_notes":null,"completion_status":"ok","verification_evidence":{{"sample_blocks_checked":8,"sample_blocks_anomalies":0,"checksum_algorithm":"sha256","verification_tool":"router-test","operator_id":"tester"}}}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("unsupported_session_schema_version")
        );
    }

    #[tokio::test]
    async fn router_offline_ingest_rejects_unsupported_result_schema_version() {
        reset_guard_limiter();

        let session_id = unique_id("session-result-schema");
        let session_dir = "data/wipe_sessions";
        let session_path = format!("{}/{}.json", session_dir, session_id);
        fs::create_dir_all(session_dir).expect("failed to create sessions dir");

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
            "phase": "wiping",
            "progress_percent": 70,
            "resume_required": true,
            "resume_hint": "awaiting_offline_result_ingest"
        });
        fs::write(
            &session_path,
            serde_json::to_string_pretty(&manifest).expect("failed to serialize manifest"),
        )
        .expect("failed to write session manifest");

        let payload = format!(
            r#"{{"schema_version":999,"session_id":"{}","target_device_id":"disk1","target_device_model":"model1","target_device_size_gb":100,"verification_passed":true,"verification_notes":null,"completion_status":"ok"}}"#,
            manifest["session_id"].as_str().expect("session_id missing")
        );
        let (status, body) = post_json("/api/offline/result/ingest", &payload).await;

        let _ = fs::remove_file(&session_path);

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("code").and_then(|v| v.as_str()),
            Some("unsupported_offline_result_schema_version")
        );
    }
