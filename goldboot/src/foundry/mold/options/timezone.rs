use crate::cli::prompt::Prompt;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Timezone {
    // TODO
}

impl Prompt for Timezone {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<()> {
        todo!()
    }
}
