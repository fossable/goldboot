use crate::{cli::prompt::Prompt, foundry::ssh::SshConnection};
use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{error::Error, path::Path, process::Command};
use validator::Validate;

/// Runs an Ansible playbook on the image remotely.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Ansible {
    /// The playbook file
    pub playbook: String,

    /// The inventory file
    pub inventory: Option<String>,
}

impl Ansible {
    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running ansible provisioner");

        if let Some(code) = Command::new("ansible-playbook")
            .arg("--ssh-common-args")
            .arg("-o StrictHostKeyChecking=no")
            .arg("-e")
            .arg(format!("ansible_port={}", ssh.port))
            .arg("-e")
            .arg(format!("ansible_user={}", ssh.username))
            .arg("-e")
            .arg(format!("ansible_ssh_pass={}", ssh.password))
            .arg("-e")
            .arg("ansible_connection=ssh")
            .arg(&self.playbook)
            .status()
            .expect("Failed to launch ansible-playbook")
            .code()
        {
            if code != 0 {
                bail!("Provisioning failed");
            }
        }

        Ok(())
    }
}

impl Prompt for Ansible {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<(), Box<dyn Error>> {
        self.playbook = dialoguer::Input::with_theme(&theme)
            .with_prompt("Enter the playbook path relative to the current directory")
            .interact()?;

        if !Path::new(&self.playbook).exists() {
            if !dialoguer::Confirm::with_theme(&theme)
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
