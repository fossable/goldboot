use crate::{cli::prompt::Prompt, foundry::Foundry};
use anyhow::Result;
use dialoguer::theme::Theme;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Timezone {
    // TODO
}

impl Prompt for Timezone {
    fn prompt(&mut self, _: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        todo!()
    }
}
