use crate::{cli::prompt::Prompt, foundry::Foundry};
use anyhow::Result;
use dialoguer::theme::Theme;
use serde::{Deserialize, Serialize};
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

// impl LoadSource for IsoSource {
//     /// Load the ISO into the cache and return its path
//     fn load(&self) -> Result<String> {
//         todo!()
//     }
// }

impl Prompt for IsoSource {
    fn prompt(&mut self, _: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        self.url = dialoguer::Input::with_theme(&*theme)
            .with_prompt("Enter the ISO URL")
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
