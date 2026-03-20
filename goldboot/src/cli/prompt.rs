use anyhow::Result;

use crate::builder::Builder;

/// Prompt the user for additional information on the command line.
pub trait Prompt {
    fn prompt(&mut self, builder: &Builder) -> Result<()>;
}
