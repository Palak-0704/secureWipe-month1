// SecureWipe API server binary
// Runs the Axum REST API for frontend-backend integration

use securewipe_core::api_router;
use std::process::Command;

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            matches!(norm.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}

fn parse_env_u16(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(default)
}

fn runtime_environment() -> String {
    std::env::var("SECUREWIPE_ENV")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "development".to_string())
}

fn has_elevated_permissions() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$p=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent()); if ($p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { 'true' } else { 'false' }",
            ])
            .output();

        return output
            .ok()
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .eq_ignore_ascii_case("true")
            })
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("id").arg("-u").output();
        return output
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|uid| uid.trim() == "0")
            .unwrap_or(false);
    }
}

fn should_fail_boot_for_missing_elevated_permissions(is_elevated: bool) -> bool {
    if is_elevated {
        return false;
    }

    if parse_env_bool("SECUREWIPE_ALLOW_UNPRIVILEGED_START", false) {
        return false;
    }

    let env = runtime_environment();
    if env == "production" {
        return true;
    }

    // In non-production, only hard-block when destructive mode is explicitly armed.
    parse_env_bool("ENABLE_REAL_ERASE", false)
        || parse_env_bool("SECUREWIPE_OFFLINE_CONFIRMED", false)
}

fn strict_targeting_enabled_from_env() -> bool {
    std::env::var("SECUREWIPE_STRICT_TARGETING")
        .ok()
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            !matches!(norm.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true)
}

fn target_allowlist_from_env() -> Vec<String> {
    std::env::var("SECUREWIPE_TARGET_ALLOWLIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn usb_real_allowlist_from_env() -> Vec<String> {
    std::env::var("SECUREWIPE_USB_REAL_ALLOWLIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn usb_real_provision_command_configured() -> bool {
    [
        "SECUREWIPE_USB_REAL_PROVISION_COMMAND",
        "SECUREWIPE_USB_PROVISION_COMMAND",
    ]
    .iter()
    .any(|name| {
        std::env::var(name)
            .ok()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    })
}

fn unknown_detection_override_enabled_from_env() -> bool {
    parse_env_bool("SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE", false)
        || parse_env_bool("SECUREWIPE_ALLOW_UNKNOWN_DETECTION", false)
}

fn should_fail_boot_for_unsafe_targeting(strict_targeting_enabled: bool) -> bool {
    if strict_targeting_enabled {
        return false;
    }

    let env = runtime_environment();
    if env != "production" {
        return false;
    }

    let unsafe_boot_allowed = parse_env_bool("SECUREWIPE_ALLOW_UNSAFE_BOOT", false);
    !unsafe_boot_allowed
}

fn should_fail_boot_for_unknown_detection_override(override_enabled: bool) -> bool {
    if !override_enabled {
        return false;
    }

    let env = runtime_environment();
    if env != "production" {
        return false;
    }

    let unsafe_boot_allowed = parse_env_bool("SECUREWIPE_ALLOW_UNSAFE_BOOT", false);
    !unsafe_boot_allowed
}

fn cors_layer_from_env() -> tower_http::cors::CorsLayer {
    use axum::http::HeaderValue;
    use tower_http::cors::{Any, CorsLayer};

    if parse_env_bool("SECUREWIPE_CORS_ALLOW_ANY", false) {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    let default_origins = ["http://localhost:5173", "http://127.0.0.1:5173"];
    let origins_var = std::env::var("SECUREWIPE_CORS_ORIGINS")
        .unwrap_or_else(|_| default_origins.join(","));

    let origins: Vec<HeaderValue> = origins_var
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect();

    let effective_origins = if origins.is_empty() {
        default_origins
            .iter()
            .filter_map(|s| s.parse::<HeaderValue>().ok())
            .collect::<Vec<_>>()
    } else {
        origins
    };

    CorsLayer::new()
        .allow_origin(effective_origins)
        .allow_methods(Any)
        .allow_headers(Any)
}

#[tokio::main]
async fn main() {
    // Start device monitoring for real-time device detection
    #[cfg(target_os = "windows")]
    securewipe_core::platform::imp::start_device_monitoring(5);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    securewipe_core::platform::start_device_monitoring(5);

    let cors = cors_layer_from_env();
    let app = api_router().layer(cors);

    let strict_targeting_enabled = strict_targeting_enabled_from_env();
    let target_allowlist = target_allowlist_from_env();
    let usb_real_allowlist = usb_real_allowlist_from_env();
    let unknown_detection_override_enabled = unknown_detection_override_enabled_from_env();
    let elevated_permissions = has_elevated_permissions();
    let usb_provision_mode_real = std::env::var("SECUREWIPE_USB_PROVISION_MODE")
        .ok()
        .map(|v| v.trim().eq_ignore_ascii_case("real"))
        .unwrap_or(false);
    let usb_real_provision_enabled = parse_env_bool("SECUREWIPE_USB_REAL_PROVISION_ENABLED", false);
    let usb_real_breakglass_enabled = parse_env_bool("SECUREWIPE_USB_REAL_BREAKGLASS", false);
    let usb_real_command_configured = usb_real_provision_command_configured();

    if should_fail_boot_for_missing_elevated_permissions(elevated_permissions) {
        eprintln!(
            "[SAFETY] Startup blocked: elevated Administrator/root permissions are required for this configuration."
        );
        eprintln!(
            "[SAFETY] For controlled development only, set SECUREWIPE_ALLOW_UNPRIVILEGED_START=1 to bypass this check."
        );
        std::process::exit(1);
    }

    if elevated_permissions {
        println!("[SAFETY] Elevated permission check: running with Administrator/root privileges.");
    } else {
        println!(
            "[SAFETY] Elevated permission check: running without Administrator/root privileges. Destructive paths remain safety-gated."
        );
    }

    if should_fail_boot_for_unsafe_targeting(strict_targeting_enabled) {
        eprintln!(
            "[SAFETY] Startup blocked: SECUREWIPE_ENV=production requires SECUREWIPE_STRICT_TARGETING to stay enabled."
        );
        eprintln!(
            "[SAFETY] To proceed only for controlled emergency debugging, set SECUREWIPE_ALLOW_UNSAFE_BOOT=1."
        );
        std::process::exit(1);
    }

    if should_fail_boot_for_unknown_detection_override(unknown_detection_override_enabled) {
        eprintln!(
            "[SAFETY] Startup blocked: SECUREWIPE_ENV=production does not allow unknown detection confidence override by default."
        );
        eprintln!(
            "[SAFETY] To proceed only for controlled emergency debugging, set SECUREWIPE_ALLOW_UNSAFE_BOOT=1."
        );
        std::process::exit(1);
    }

    let bind_host = std::env::var("SECUREWIPE_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let bind_port = parse_env_u16("SECUREWIPE_BIND_PORT", 8080);
    let addr = format!("{}:{}", bind_host, bind_port)
        .parse::<std::net::SocketAddr>()
        .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 8080)));

    if strict_targeting_enabled {
        if target_allowlist.is_empty() {
            println!(
                "[SAFETY] Strict destructive targeting is ENABLED (default): only removable devices are eligible; no allowlist IDs configured."
            );
        } else {
            println!(
                "[SAFETY] Strict destructive targeting is ENABLED: removable devices or allowlisted IDs are eligible ({} IDs loaded).",
                target_allowlist.len()
            );
            println!(
                "[SAFETY] Loaded destructive target allowlist IDs: {}",
                target_allowlist.join(", ")
            );
        }
    } else {
        println!(
            "[SAFETY] Strict destructive targeting is DISABLED by SECUREWIPE_STRICT_TARGETING; rely on protected-system and runtime confirmation guards only."
        );
        if runtime_environment() != "production" {
            println!(
                "[SAFETY] Non-production override active. For production, this configuration is blocked unless SECUREWIPE_ALLOW_UNSAFE_BOOT=1 is set."
            );
        }
    }

    if unknown_detection_override_enabled {
        println!(
            "[SAFETY] Unknown detection confidence override is ENABLED; destructive flows may proceed with reduced detection certainty."
        );
        if runtime_environment() != "production" {
            println!(
                "[SAFETY] Non-production override active for detection confidence policy."
            );
        }
    } else {
        println!(
            "[SAFETY] Unknown detection confidence override is DISABLED (default)."
        );
    }

    println!(
        "[SAFETY] USB real provisioning policy: mode_real={}, real_exec_enabled={}, breakglass_enabled={}, allowlist_count={}, command_configured={}",
        usb_provision_mode_real,
        usb_real_provision_enabled,
        usb_real_breakglass_enabled,
        usb_real_allowlist.len(),
        usb_real_command_configured
    );
    if usb_real_provision_enabled && !usb_real_breakglass_enabled {
        println!(
            "[SAFETY] USB real provisioning command execution is enabled but breakglass is OFF; API policy still blocks real mode requests."
        );
    }

    println!("SecureWipe API server running at http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
