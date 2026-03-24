use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A list of packages to install.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Packages(pub Vec<String>);

impl Prompt for Packages {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::{Confirm, Input};
        let theme = crate::cli::cmd::init::theme();

        loop {
            let pkg: String = Input::with_theme(&theme)
                .with_prompt("Package to install (leave blank to finish)")
                .allow_empty(true)
                .interact_text()?;

            if pkg.is_empty() {
                break;
            }

            self.0.push(pkg);

            if !Confirm::with_theme(&theme)
                .with_prompt("Add another package?")
                .default(true)
                .interact()?
            {
                break;
            }
        }

        Ok(())
    }
}
