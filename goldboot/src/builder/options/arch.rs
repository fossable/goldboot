use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Arch(pub ImageArch);

impl Prompt for Arch {
    fn prompt(&mut self, builder: &Builder) -> Result<()> {
        todo!()
    }
}
