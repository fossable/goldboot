use crate::{cli::prompt::Prompt, foundry::Foundry};
use anyhow::Result;
use dialoguer::{theme::Theme, Confirm, Password};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Configures a LUKS encrypted root filesystem
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Luks {
    /// The LUKS passphrase
    pub passphrase: String,

    /// Whether the LUKS passphrase will be enrolled in a TPM
    pub tpm: bool,
}

impl Prompt for Luks {
    fn prompt(&mut self, _: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        if Confirm::with_theme(&*theme)
            .with_prompt("Do you want to encrypt the root partition with LUKS?")
            .interact()?
        {
            self.passphrase = Password::with_theme(&*theme)
                .with_prompt("LUKS passphrase")
                .interact()?;
        }

        self.validate()?;
        Ok(())
    }
}
