use crate::packer::PackerTemplate;
use std::{error::Error, path::Path};

/// Represents a "base configuration" that users can modify and use to build images.
pub trait Profile {
    /// Generate a packer template
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>>;
}

pub fn list_profiles() -> Result<(), Box<dyn Error>> {
    Ok(())
}
