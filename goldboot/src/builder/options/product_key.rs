use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// A product key used to activate the OS (e.g. a Windows product key).
#[derive(Clone, Serialize, Deserialize, Debug, SmartDefault)]
pub struct ProductKey(pub String);

impl Prompt for ProductKey {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::Input;
        let theme = crate::cli::cmd::init::theme();

        self.0 = Input::with_theme(&theme)
            .with_prompt("Product key")
            .allow_empty(true)
            .interact_text()?;

        Ok(())
    }
}
