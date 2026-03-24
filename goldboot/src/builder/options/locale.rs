use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use validator::Validate;

/// Locale and keyboard settings.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct Locale {
    /// Keyboard layout (e.g. "us", "de")
    #[default("us".to_string())]
    pub keyboard: String,
    /// System language (e.g. "en_US")
    #[default("en_US".to_string())]
    pub language: String,
    /// System encoding (e.g. "UTF-8")
    #[default("UTF-8".to_string())]
    pub encoding: String,
}

impl Prompt for Locale {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::Input;
        let theme = crate::cli::cmd::init::theme();

        self.keyboard = Input::with_theme(&theme)
            .with_prompt("Keyboard layout")
            .default(self.keyboard.clone())
            .interact_text()?;

        self.language = Input::with_theme(&theme)
            .with_prompt("System language")
            .default(self.language.clone())
            .interact_text()?;

        self.encoding = Input::with_theme(&theme)
            .with_prompt("System encoding")
            .default(self.encoding.clone())
            .interact_text()?;

        Ok(())
    }
}
