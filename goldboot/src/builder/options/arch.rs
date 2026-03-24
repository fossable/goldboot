use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Arch(pub ImageArch);

impl Prompt for Arch {
    fn prompt(&mut self, _builder: &Builder) -> Result<()> {
        use dialoguer::Select;
        let theme = crate::cli::cmd::init::theme();

        let options = [
            ImageArch::Amd64,
            ImageArch::Arm64,
            ImageArch::I386,
            ImageArch::Mips,
            ImageArch::Mips64,
            ImageArch::S390x,
        ];
        let labels = ["amd64", "arm64", "i386", "mips", "mips64", "s390x"];

        let current = options.iter().position(|a| *a == self.0).unwrap_or(0);

        let selection = Select::with_theme(&theme)
            .with_prompt("Architecture")
            .items(&labels)
            .default(current)
            .interact()?;

        self.0 = options[selection].clone();
        Ok(())
    }
}
