use chrono::Utc;
use serde::Serialize;
use std::process::Command;

use super::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsbProvisionMode {
    Simulation,
    Real,
}

impl UsbProvisionMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Simulation => "simulation",
            Self::Real => "real",
        }
    }
}

#[derive(Serialize)]
struct UsbProvisionReport {
    mode: String,
    usb_device_id: String,
    output_path: String,
    bootable_verified: bool,
    command: Option<String>,
    args: Vec<String>,
    status: String,
    detail: String,
    stdout: Option<String>,
    stderr: Option<String>,
    generated_at: String,
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            matches!(norm.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}

fn first_nonempty_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

fn log_usb_imaging_event(event: &str, usb_device_id: &str, detail: &str) {
    println!(
        "[USB_IMAGING] event={} usb_device_id={} detail={}",
        event,
        usb_device_id,
        detail
    );
}

fn command_args_from_env() -> Result<Vec<String>, AppError> {
    let raw = first_nonempty_env(&[
        "SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON",
        "SECUREWIPE_USB_PROVISION_ARGS_JSON",
    ])
    .unwrap_or_else(|| "[]".to_string());
    serde_json::from_str::<Vec<String>>(&raw).map_err(|_| {
        AppError::bad_request(
            "usb_provision_args_invalid",
            "SECUREWIPE_USB_REAL_PROVISION_ARGS_JSON must be a JSON array of strings.",
        )
    })
}

fn resolve_usb_provision_args(args: &[String], usb_device_id: &str, output_path: &str) -> Vec<String> {
    args.iter()
        .map(|a| {
            a.replace("{usb_device_id}", usb_device_id)
                .replace("{output_path}", output_path)
        })
        .collect()
}

fn write_usb_provision_report(output_path: &str, report: &UsbProvisionReport) -> Result<String, AppError> {
    let report_path = format!("{}/USB_PROVISION_REPORT.json", output_path);
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(report).map_err(|_| {
            AppError::internal_server_error(
                "usb_provision_report_serialize_failed",
                "Failed to serialize USB provision report.",
            )
        })?,
    )
    .map_err(|_| {
        AppError::internal_server_error(
            "usb_provision_report_write_failed",
            "Failed to write USB provision report.",
        )
    })?;

    Ok(report_path)
}

pub(crate) fn run_usb_provisioning(
    mode: UsbProvisionMode,
    usb_device_id: &str,
    output_path: &str,
) -> Result<(bool, String), AppError> {
    log_usb_imaging_event(
        "provisioning_requested",
        usb_device_id,
        &format!("mode={} output_path={}", mode.as_str(), output_path),
    );

    if mode == UsbProvisionMode::Simulation {
        let report = UsbProvisionReport {
            mode: mode.as_str().to_string(),
            usb_device_id: usb_device_id.to_string(),
            output_path: output_path.to_string(),
            bootable_verified: true,
            command: None,
            args: vec![],
            status: "simulated".to_string(),
            detail: "USB provisioning simulated. No destructive USB writes were performed.".to_string(),
            stdout: None,
            stderr: None,
            generated_at: Utc::now().to_rfc3339(),
        };
        let report_path = write_usb_provision_report(output_path, &report)?;
        log_usb_imaging_event("provisioning_simulated", usb_device_id, &format!("report_path={}", report_path));
        return Ok((true, report_path));
    }

    if !parse_env_bool("SECUREWIPE_USB_REAL_PROVISION_ENABLED", false) {
        return Err(AppError::forbidden(
            "usb_real_provisioning_not_enabled",
            "Real USB provisioning is disabled. Set SECUREWIPE_USB_REAL_PROVISION_ENABLED=1 for controlled lab usage.",
        ));
    }

    let command = first_nonempty_env(&[
        "SECUREWIPE_USB_REAL_PROVISION_COMMAND",
        "SECUREWIPE_USB_PROVISION_COMMAND",
    ])
    .ok_or_else(|| {
        AppError::unprocessable_entity(
            "usb_real_provision_command_missing",
            "SECUREWIPE_USB_REAL_PROVISION_COMMAND is required when SECUREWIPE_USB_PROVISION_MODE=real.",
        )
    })?;
    let args_template = command_args_from_env()?;
    let args = resolve_usb_provision_args(&args_template, usb_device_id, output_path);

    let output = Command::new(&command)
        .args(&args)
        .output()
        .map_err(|_| {
            AppError::internal_server_error(
                "usb_provision_command_start_failed",
                "Failed to start USB provisioning command.",
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let bootable_verified = output.status.success();

    let report = UsbProvisionReport {
        mode: mode.as_str().to_string(),
        usb_device_id: usb_device_id.to_string(),
        output_path: output_path.to_string(),
        bootable_verified,
        command: Some(command.clone()),
        args,
        status: if output.status.success() {
            "completed".to_string()
        } else {
            "failed".to_string()
        },
        detail: if output.status.success() {
            "USB provisioning command completed successfully.".to_string()
        } else {
            "USB provisioning command failed.".to_string()
        },
        stdout: if stdout.is_empty() { None } else { Some(stdout.clone()) },
        stderr: if stderr.is_empty() { None } else { Some(stderr.clone()) },
        generated_at: Utc::now().to_rfc3339(),
    };
    let report_path = write_usb_provision_report(output_path, &report)?;
    log_usb_imaging_event(
        if output.status.success() {
            "provisioning_completed"
        } else {
            "provisioning_failed"
        },
        usb_device_id,
        &format!("command={} report_path={}", command, report_path),
    );

    if !output.status.success() {
        return Err(AppError::conflict(
            "usb_provisioning_failed",
            format!(
                "USB provisioning command failed. stderr: {}",
                if stderr.is_empty() { "(empty)" } else { stderr.as_str() }
            ),
        ));
    }

    Ok((bootable_verified, report_path))
}
