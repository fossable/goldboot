use std::{error::Error, path::Path};

/// Generate a config for the current hardware
pub fn generate_qemuargs() -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    Ok(vec![vec![]])
}