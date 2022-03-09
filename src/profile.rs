use crate::config::Config;
use crate::packer::PackerTemplate;
use std::{error::Error, path::Path};

/// Represents a "base configuration" that users can modify and use to build images.
pub trait Profile {
    /// This hook is invoked during the build step.
    fn build(&self, template: &mut PackerTemplate, context: &Path) -> Result<(), Box<dyn Error>>;
}
