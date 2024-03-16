use anyhow::Result;
use dialoguer::theme::Theme;
use enum_dispatch::enum_dispatch;

use crate::foundry::Foundry;

/// Prompt the user for additional information on the command line.
#[enum_dispatch(ImageMold)]
pub trait Prompt {
    fn prompt(&mut self, foundry: &Foundry, theme: Box<dyn Theme>) -> Result<()>;
}

/// Prompt the user for additional information on the command line.
pub trait PromptNew {
    fn prompt_new(foundry: &Foundry, theme: Box<dyn Theme>) -> Result<Self>
    where
        Self: Sized;
}
