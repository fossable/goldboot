use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use dialoguer::Password;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use validator::Validate;

/// Configures a LUKS encrypted root filesystem.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct Luks {
    /// The LUKS passphrase
    pub passphrase: String,

    /// Whether the LUKS passphrase will be enrolled in a TPM
    pub tpm: bool,
}

impl Prompt for Luks {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let theme = crate::cli::cmd::init::theme();

        self.passphrase = Password::with_theme(&theme)
            .with_prompt("LUKS passphrase")
            .interact()?;

        self.validate()?;
        Ok(())
    }
}
