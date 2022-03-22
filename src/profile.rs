use crate::config::Config;
use std::error::Error;

/// Represents a "base configuration" that users can modify and use to build images.
pub trait Profile {
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>>;
}

pub fn list_profiles() -> Result<(), Box<dyn Error>> {
    Ok(())
}
