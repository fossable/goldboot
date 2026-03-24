use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Whether to enable NTP time synchronization.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Ntp(pub bool);

impl Default for Ntp {
    fn default() -> Self {
        Self(true)
    }
}

impl Prompt for Ntp {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::Confirm;
        let theme = crate::cli::cmd::init::theme();

        self.0 = Confirm::with_theme(&theme)
            .with_prompt("Enable NTP time synchronization?")
            .default(self.0)
            .interact()?;

        Ok(())
    }
}
