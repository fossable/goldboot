use std::error::Error;

use dialoguer::{theme::ColorfulTheme, Confirm, Password};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{build::BuildConfig, PromptMut};

/// Configures a LUKS encrypted root filesystem
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Luks {
    /// The LUKS passphrase
    pub passphrase: String,

    /// Whether the LUKS passphrase will be enrolled in a TPM
    pub tpm: bool,
}

impl PromptMut for Luks {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        if Confirm::with_theme(theme)
            .with_prompt("Do you want to encrypt the root partition with LUKS?")
            .interact()?
        {
            self.passphrase = Password::with_theme(theme)
                .with_prompt("LUKS passphrase")
                .interact()?;
        }

        self.validate()?;
        Ok(())
    }
}
