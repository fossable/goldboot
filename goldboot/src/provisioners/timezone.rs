#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct TimezoneProvisioner {
    // TODO
}

impl PromptMut for TimezoneProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}
