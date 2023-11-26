use crate::cli::prompt::Prompt;
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

/// Sets the network hostname.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Hostname {
    // TODO validate
    pub hostname: String,
}

impl Default for Hostname {
    fn default() -> Self {
        Self {
            hostname: String::from("goldboot"),
        }
    }
}

impl Prompt for Hostname {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<(), Box<dyn Error>> {
        self.hostname = dialoguer::Input::with_theme(&theme)
            .with_prompt("Enter network hostname")
            .default(config.name.clone())
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
