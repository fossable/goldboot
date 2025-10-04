use super::Fabricate;
use crate::foundry::Foundry;
use crate::{cli::prompt::Prompt, foundry::ssh::SshConnection};
use anyhow::Result;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;
use validator::Validate;

/// Runs an executable file.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct HostExecutable {
    /// The path to the executable
    pub path: String,
}

impl Fabricate for HostExecutable {
    fn run(&self, ssh: &mut SshConnection) -> Result<()> {
        info!("Running executable");

        if ssh.upload_exec(&std::fs::read(&self.path)?, vec![])? != 0 {
            bail!("Executable failed");
        }
        Ok(())
    }
}

impl Prompt for HostExecutable {
    fn prompt(&mut self, _: &Foundry) -> Result<()> {
        self.path = dialoguer::Input::with_theme(&crate::cli::cmd::init::theme())
            .with_prompt("Enter the script path relative to the current directory")
            .interact()?;

        if !Path::new(&self.path).exists() {
            if !dialoguer::Confirm::with_theme(&crate::cli::cmd::init::theme())
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
