use crate::{cli::prompt::Prompt, builder::Foundry};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Timezone {
    // TODO
}

impl Prompt for Timezone {
    fn prompt(&mut self, _: &Foundry) -> Result<()> {
        todo!()
    }
}
