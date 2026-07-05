use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// System timezone in tz database format (e.g. "UTC", "America/New_York").
#[derive(Clone, Serialize, Deserialize, Debug, SmartDefault)]
pub struct Timezone(#[default("UTC".to_string())] pub String);

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
