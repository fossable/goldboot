use anyhow::Result;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct QemuConfig {

	pub memory: String,

	pub bios: String,
}

impl QemuConfig {
	pub fn to_qemuargs(&self) -> Vec<Vec<String>> {
		vec![
			vec!["-m".to_string(), self.memory.clone()],
		]
	}
}

/// Generate a config for the current hardware
pub fn generate_config() -> Result<()> {
	Ok(())
}
