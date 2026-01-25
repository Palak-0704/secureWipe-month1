//! src/engine/wipe.rs
//!
//! Wiping method stub for Month 1 submission.
///
/// Simulates a secure wipe on the given [Device]. Returns a result string.

use crate::devices::Device;

pub fn perform_wipe(device: &Device) -> String {
    format!("Simulated wipe performed on device: {} ({})", device.model, device.id)
}
