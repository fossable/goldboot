use std::{error::Error, path::Path};

use dialoguer::theme::ColorfulTheme;
use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use validator::Validate;

use crate::{build::BuildConfig, ssh::SshConnection, PromptMut};

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ExecutableProvisioners {
    pub executables: Vec<ExecutableProvisioner>,
}

/// This provisioner runs an executable file on the image.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ExecutableProvisioner {
    /// The path to the executable
    pub path: String,

    /// Overrides the default run order
    pub order: Option<usize>,
}

impl ExecutableProvisioner {
    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running executable provisioner");

        if ssh.upload_exec(&std::fs::read(self.path.clone())?, vec![])? != 0 {
            bail!("Provisioner failed");
        }
        Ok(())
    }
}

impl PromptMut for ExecutableProvisioner {
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
