use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// System timezone in tz database format (e.g. "UTC", "America/New_York").
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Timezone(pub String);

impl Default for Timezone {
    fn default() -> Self {
        Self("UTC".to_string())
    }
}

impl Prompt for Timezone {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::Input;
        let theme = crate::cli::cmd::init::theme();

        self.0 = Input::with_theme(&theme)
            .with_prompt("Timezone")
            .default(self.0.clone())
            .interact_text()?;

        Ok(())
    }
}
