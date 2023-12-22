use anyhow::Result;

pub trait Prompt {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<()>;
}

pub trait PromptNew {
    fn prompt(config: &BuildConfig, theme: Box<dyn dialoguer::theme::Theme>) -> Result<Self>
    where
        Self: Sized;
}
