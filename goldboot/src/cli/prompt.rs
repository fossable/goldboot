use anyhow::Result;
use enum_dispatch::enum_dispatch;

use crate::foundry::Foundry;

/// Prompt the user for additional information on the command line.
#[enum_dispatch(Os)]
pub trait Prompt {
    fn prompt(&mut self, foundry: &Foundry) -> Result<()>;
}
