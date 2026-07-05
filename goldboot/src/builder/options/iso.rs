use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;
use validator::Validate;

/// Use an ISO image as a source.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Iso {
    /// The installation media URL (http, https, or file)
    pub url: Url,

    /// A hash of the installation media
    pub checksum: Option<String>,
}

impl Prompt for Iso {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::Input;
        let theme = crate::cli::cmd::init::theme();

        self.url = Input::with_theme(&theme)
            .with_prompt("Enter the ISO URL")
            .interact()?;

        let checksum: String = Input::with_theme(&theme)
            .with_prompt("Enter the ISO checksum (e.g. sha256:..., leave blank to skip)")
            .allow_empty(true)
            .interact_text()?;
        self.checksum = if checksum.is_empty() {
            None
        } else {
            Some(checksum)
        };

        self.validate()?;
        Ok(())
    }
}
