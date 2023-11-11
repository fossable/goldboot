use std::error::Error;

use dialoguer::theme::ColorfulTheme;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{build::BuildConfig, PromptMut};

/// This provisioner changes the network hostname.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct HostnameProvisioner {
    // TODO validate
    pub hostname: String,
}

impl Default for HostnameProvisioner {
    fn default() -> Self {
        Self {
            hostname: String::from("goldboot"),
        }
    }
}

impl PromptMut for HostnameProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.hostname = dialoguer::Input::with_theme(theme)
            .with_prompt("Enter network hostname")
            .default(config.name.clone())
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
