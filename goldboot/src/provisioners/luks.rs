/// This provisioner configures a LUKS encrypted root filesystem
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct LuksProvisoner {
    /// The LUKS passphrase
    pub passphrase: String,

    /// Whether the LUKS passphrase will be enrolled in a TPM
    pub tpm: bool,
}

impl PromptMut for LuksProvisoner {
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
