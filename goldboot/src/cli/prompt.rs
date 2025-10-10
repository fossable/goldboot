use anyhow::Result;
use enum_dispatch::enum_dispatch;

use crate::builder::Builder;

/// Prompt the user for additional information on the command line.
#[enum_dispatch(Os)]
pub trait Prompt {
    fn prompt(&mut self, builder: &Builder) -> Result<()>;
}
