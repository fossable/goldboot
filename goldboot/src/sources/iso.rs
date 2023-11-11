use super::Source;
use crate::{build::BuildConfig, PromptMut};
use dialoguer::theme::ColorfulTheme;
use serde::{Deserialize, Serialize};
use std::error::Error;
use url::Url;
use validator::Validate;

/// Uses an ISO image as a source.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct IsoSource {
    /// The installation media URL (http, https, or file)
    pub url: Url,

    /// A hash of the installation media
    pub checksum: Option<String>,
}

impl Source for IsoSource {
    /// Load the ISO into the cache and return its path
    fn load(&self) -> Result<String, Box<dyn Error>> {
        todo!()
    }
}

impl PromptMut for IsoSource {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.url = dialoguer::Input::with_theme(theme)
            .with_prompt("Enter the ISO URL")
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
