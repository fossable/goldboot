use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct QemuConfig {
    #[serde(default)]
    pub bios: String,
}

impl QemuConfig {
    pub fn to_qemuargs(&self) -> Vec<Vec<String>> {
        vec![vec!["-bios".into(), self.bios.clone()]]
    }

    /// Generate a config for the current hardware
    pub fn generate_config() -> Result<QemuConfig, Box<dyn Error>> {
        let mut qemu_config = QemuConfig::default();
        Ok(qemu_config)
    }
}
