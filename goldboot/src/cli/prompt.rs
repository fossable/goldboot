use anyhow::Result;
use dialoguer::theme::Theme;

use crate::foundry::Foundry;

/// Prompt the user for additional information on the command line.
pub trait Prompt {
    fn prompt(&mut self, foundry: &Foundry, theme: Box<dyn Theme>) -> Result<()>;
}

/// Prompt the user for additional information on the command line.
pub trait PromptNew {
    fn prompt(foundry: &Foundry, theme: Box<dyn Theme>) -> Result<Self>
    where
        Self: Sized;
}
