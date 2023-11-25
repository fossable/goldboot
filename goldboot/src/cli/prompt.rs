use std::error::Error;

pub trait Prompt {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<(), Box<dyn Error>>;
}

pub trait PromptNew {
    fn prompt(
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
}
