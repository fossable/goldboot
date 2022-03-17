use crate::ovmf_firmware;
use std::error::Error;

/// Generate a config for the current hardware
pub fn generate_qemuargs() -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let mut qemuargs: Vec<Vec<String>> = Vec::new();

    qemuargs.push(vec![
        String::from("-bios"),
        ovmf_firmware().ok_or("Failed to locate firmware")?,
    ]);

    // TODO get CPU type, etc

    return Ok(qemuargs);
}
