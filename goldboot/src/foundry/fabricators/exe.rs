use std::{error::Error, path::Path};

use dialoguer::theme::ColorfulTheme;
use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use validator::Validate;

use crate::{build::BuildConfig, ssh::SshConnection, PromptMut};

use super::Fabricator;

/// Runs an executable file.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ExecutableFabricator {
    /// The path to the executable
    pub path: String,
}

impl ExecutableFabricator {
    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running executable");

        if ssh.upload_exec(&std::fs::read(self.path.clone())?, vec![])? != 0 {
            bail!("Executable failed");
        }
        Ok(())
    }
}

impl PromptMut for ExecutableFabricator {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.path = dialoguer::Input::with_theme(theme)
            .with_prompt("Enter the script path relative to the current directory")
            .interact()?;

        if !Path::new(&self.path).exists() {
            if !dialoguer::Confirm::with_theme(theme)
                .with_prompt("The path does not exist. Add anyway?")
                .interact()?
            {
                bail!("The playbook did not exist");
            }
        }

        self.validate()?;
        Ok(())
    }
}

impl Fabricator for ExecutableFabricator;
