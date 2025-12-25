use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;
use validator::Validate;

/// Use an ISO image as a source.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, goldboot_macros::StarlarkConstructor)]
pub struct Iso {
    /// The installation media URL (http, https, or file)
    pub url: Url,

    /// A hash of the installation media
    pub checksum: Option<String>,
}

impl Prompt for Iso {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        self.url = dialoguer::Input::with_theme(&crate::cli::cmd::init::theme())
            .with_prompt("Enter the ISO URL")
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
