use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct QemuConfig {
    #[serde(default)]
    pub bios: Option<String>,

    #[serde(default)]
    pub pflash: Option<String>,
}

impl QemuConfig {
    pub fn to_qemuargs(&self) -> Vec<Vec<String>> {
        let mut args: Vec<Vec<String>> = Vec::new();

        if let Some(bios) = &self.bios {
            //args.push(vec![String::from("-bios"), String::from(bios)]);
        }

        if let Some(pflash) = &self.pflash {
            args.push(vec![
                String::from("-drive"),
                String::from(
                    "if=pflash,format=raw,unit=0,readonly,file=/usr/share/ovmf/x64/OVMF.fd",
                ),
            ]);
            args.push(vec![
                String::from("-drive"),
                String::from("if=pflash,format=raw,unit=1,file=/tmp/OVMF_VARS.fd"),
            ]);
        }

        args
    }

    /// Generate a config for the current hardware
    pub fn generate_config() -> Result<QemuConfig, Box<dyn Error>> {
        let mut qemu_config = QemuConfig::default();

        // Search for UEFI firmware
        if Path::new("/usr/share/ovmf/x64/OVMF.fd").is_file() {
            qemu_config.bios = Some(String::from("/usr/share/ovmf/x64/OVMF.fd"));
            qemu_config.pflash = Some(String::from("/tmp/test.fd"));
        }

        Ok(qemu_config)
    }
}
