pub mod wipe_history;
/// Plugin/config support stub for future extensibility
///
/// # Arguments
/// * `_path` - Path to plugin or config file (future use)
pub fn load_custom_plugins_or_config(_path: &str) {
	// In future: dynamically load plugins or config for custom wipe/compliance logic
	// For now: no-op
}

/// See [Device] for device info, [detect_devices] for device scan, [perform_wipe] for wipe, [Recommendation] for AI/ML advice, [chatbot_groq_api_with_config] for chatbot, [sanitize_input] for input sanitization.

pub mod devices;
pub mod platform;
pub mod engine;
pub mod ai;

// REST API integration (Axum)
pub mod api;

// Re-export key structs and functions for frontend integration (FFI, Tauri, REST API, etc.)
pub use devices::{Device, detect_devices};
pub use engine::wipe::perform_wipe;
pub use ai::{Recommendation, ComplianceContext, recommend_method, chatbot_groq_api_with_config, chatbot_qa};
pub use api::api_router;
