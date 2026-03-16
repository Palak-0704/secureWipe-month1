use crate::manifest::WipeManifest;

/// Execute the wipe for the given manifest.
///
/// In a real destructive build (`ENABLE_REAL_ERASE=1`, `SECUREWIPE_RUNTIME=offline`),
/// operators should replace this stub with a real erase command invocation.
/// By default (simulation), this returns a success message without touching storage.
pub fn execute_wipe(manifest: &WipeManifest) -> Result<String, String> {
    let real_erase = is_truthy_env("ENABLE_REAL_ERASE");
    let runtime = std::env::var("SECUREWIPE_RUNTIME")
        .unwrap_or_default()
        .trim()
        .to_lowercase();

    if real_erase && runtime == "offline" {
        execute_real_wipe(manifest)
    } else {
        // Simulation path — safe by default
        Ok(format!(
            "Simulation wipe completed for device {} ({}) using method {}. \
             No storage was modified. Set ENABLE_REAL_ERASE=1 and SECUREWIPE_RUNTIME=offline \
             for a destructive run.",
            manifest.target_device_id, manifest.target_device_model, manifest.method
        ))
    }
}

fn execute_real_wipe(manifest: &WipeManifest) -> Result<String, String> {
    // Guards: must have an erase executable configured
    let executable = std::env::var("SECUREWIPE_ERASE_EXECUTABLE").map_err(|_| {
        "SECUREWIPE_ERASE_EXECUTABLE is required for real offline erase".to_string()
    })?;

    if executable.trim().is_empty() {
        return Err("SECUREWIPE_ERASE_EXECUTABLE cannot be empty".to_string());
    }

    // Resolve args template
    let args: Vec<String> = std::env::var("SECUREWIPE_ERASE_ARGS_JSON")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            serde_json::from_str::<Vec<String>>(&v)
                .map_err(|_| "SECUREWIPE_ERASE_ARGS_JSON must be a JSON array of strings".to_string())
        })
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(|arg| apply_placeholders(&arg, manifest))
        .collect();

    let output = std::process::Command::new(&executable)
        .args(&args)
        .output()
        .map_err(|e| format!("failed to start executor '{}': {}", executable, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else if !stdout.is_empty() { stdout } else { "non-zero exit, no output".to_string() };
        return Err(format!("executor reported failure: {}", detail));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if stdout.is_empty() {
        format!(
            "Real offline wipe completed for device {} ({})",
            manifest.target_device_id, manifest.target_device_model
        )
    } else {
        stdout
    })
}

fn apply_placeholders(template: &str, manifest: &WipeManifest) -> String {
    template
        .replace("{device_id}", &manifest.target_device_id)
        .replace("{device_model}", &manifest.target_device_model)
        .replace("{device_size_gb}", &manifest.target_device_size_gb.to_string())
        .replace(
            "{device_serial}",
            manifest.target_device_serial.as_deref().unwrap_or("unknown_serial"),
        )
}

fn is_truthy_env(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            let n = v.trim().to_ascii_lowercase();
            matches!(n.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}
