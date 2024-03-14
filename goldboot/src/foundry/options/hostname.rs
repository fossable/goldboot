use std::fmt::Display;

use crate::{cli::prompt::Prompt, foundry::Foundry};
use anyhow::Result;
use dialoguer::theme::Theme;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Sets the network hostname.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Hostname {
    // TODO validate
    pub hostname: String,
}

impl Display for Hostname {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hostname)
    }
}

impl Default for Hostname {
    fn default() -> Self {
        Self {
            hostname: String::from("goldboot"),
        }
    }
}

impl Prompt for Hostname {
    fn prompt(&mut self, foundry: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        self.hostname = dialoguer::Input::with_theme(&*theme)
            .with_prompt("Enter network hostname")
            .default(foundry.name.clone())
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
