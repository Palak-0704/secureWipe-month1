use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use super::errors::ApiErrorBody;

/// Bearer-token authentication middleware.
///
/// Behaviour:
/// - If `SECUREWIPE_API_TOKEN` is not set (or empty), all requests pass through — backward-compatible
///   for development environments without auth configured.
/// - If the env var is set, every request that does NOT target an exempt path must carry an
///   `Authorization: Bearer <token>` header whose value exactly matches the configured token.
///
/// Exempt paths (always pass through even when a token is required):
/// - `GET /` — API root
/// - `GET /api/system/health` — health check (must be reachable by load-balancers)
///
/// Timing-safe comparison is used to prevent oracle attacks.
pub async fn bearer_token_auth<B>(request: Request<B>, next: Next<B>) -> Response {
    let required = match std::env::var("SECUREWIPE_API_TOKEN") {
        Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
        // No token configured — open access (development default).
        _ => return next.run(request).await,
    };

    let path = request.uri().path().to_string();
    // Exempt health-check and API root from authentication.
    if path == "/" || path == "/api/system/health" {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let provided = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t.trim().to_string(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiErrorBody {
                    error: "Authorization header with a Bearer token is required.".to_string(),
                    code: "authentication_required".to_string(),
                }),
            )
                .into_response();
        }
    };

    if !constant_time_eq(provided.as_bytes(), required.as_bytes()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiErrorBody {
                error: "Invalid or expired Bearer token.".to_string(),
                code: "authentication_failed".to_string(),
            }),
        )
            .into_response();
    }

    next.run(request).await
}

/// Constant-time byte slice comparison that resists timing side-channel attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches_equal_slices() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn constant_time_eq_rejects_different_slices() {
        assert!(!constant_time_eq(b"abc", b"xyz"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"ab", b"abc"));
    }
}
