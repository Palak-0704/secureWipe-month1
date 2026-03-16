use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::timeout;

use super::errors::ApiErrorBody;

#[derive(Debug, Clone, Copy)]
struct GuardConfig {
    max_body_bytes: usize,
    max_requests_per_window: u32,
    window_seconds: i64,
    request_timeout_seconds: u64,
}

fn parse_env_usize(name: &str, default: usize, min: usize, max: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn parse_env_u32(name: &str, default: u32, min: u32, max: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn parse_env_i64(name: &str, default: i64, min: i64, max: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn parse_env_u64(name: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn guard_config() -> GuardConfig {
    GuardConfig {
        max_body_bytes: parse_env_usize(
            "SECUREWIPE_GUARD_MAX_BODY_BYTES",
            256 * 1024,
            1024,
            10 * 1024 * 1024,
        ),
        max_requests_per_window: parse_env_u32("SECUREWIPE_GUARD_RATE_LIMIT", 20, 1, 10_000),
        window_seconds: parse_env_i64("SECUREWIPE_GUARD_RATE_WINDOW_SECONDS", 60, 1, 86_400),
        request_timeout_seconds: parse_env_u64("SECUREWIPE_GUARD_TIMEOUT_SECONDS", 30, 1, 600),
    }
}

lazy_static! {
    static ref HIGH_RISK_LIMITER: Mutex<HashMap<String, RateWindow>> = Mutex::new(HashMap::new());
}

struct RateWindow {
    window_start_epoch: i64,
    count: u32,
}

fn is_high_risk_path(path: &str) -> bool {
    path.starts_with("/api/wipe/") || path.starts_with("/api/usb/") || path == "/api/offline/wipe/execute"
}

fn client_identity<B>(req: &Request<B>) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| raw.split(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown-client".to_string())
}

fn rate_limit_allow(cfg: GuardConfig, client_key: &str) -> bool {
    let now = Utc::now().timestamp();
    let mut guard = match HIGH_RISK_LIMITER.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };

    guard.retain(|_, entry| now - entry.window_start_epoch < cfg.window_seconds * 2);
    let entry = guard.entry(client_key.to_string()).or_insert(RateWindow {
        window_start_epoch: now,
        count: 0,
    });

    if entry.window_start_epoch == 0 || now - entry.window_start_epoch >= cfg.window_seconds {
        entry.window_start_epoch = now;
        entry.count = 0;
    }

    if entry.count >= cfg.max_requests_per_window {
        return false;
    }

    entry.count += 1;
    true
}

#[cfg(test)]
pub(crate) fn reset_high_risk_limiter() {
    if let Ok(mut guard) = HIGH_RISK_LIMITER.lock() {
        guard.clear();
    }
}

pub async fn high_risk_guard<B>(req: Request<B>, next: Next<B>) -> Response {
    let path = req.uri().path().to_string();
    if is_high_risk_path(&path) {
        let cfg = guard_config();
        let client_key = client_identity(&req);

        if let Some(len) = req
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
        {
            if len > cfg.max_body_bytes {
                return (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Json(ApiErrorBody {
                        error: "Request body too large for high-risk endpoint.".to_string(),
                        code: "payload_too_large".to_string(),
                    }),
                )
                    .into_response();
            }
        }

        if !rate_limit_allow(cfg, &client_key) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ApiErrorBody {
                    error: "Rate limit exceeded for high-risk endpoints.".to_string(),
                    code: "rate_limited".to_string(),
                }),
            )
                .into_response();
        }

        match timeout(Duration::from_secs(cfg.request_timeout_seconds), next.run(req)).await {
            Ok(resp) => resp,
            Err(_) => (
                StatusCode::GATEWAY_TIMEOUT,
                Json(ApiErrorBody {
                    error: "Request timed out in high-risk endpoint.".to_string(),
                    code: "request_timeout".to_string(),
                }),
            )
                .into_response(),
        }
    } else {
        next.run(req).await
    }
}
