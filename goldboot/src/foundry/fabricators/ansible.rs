use crate::foundry::Foundry;
use crate::{cli::prompt::Prompt, foundry::ssh::SshConnection};
use anyhow::bail;
use anyhow::Result;
use dialoguer::theme::Theme;
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};
use tracing::info;
use validator::Validate;

use super::Fabricate;

/// Runs an Ansible playbook on the image remotely.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Ansible {
    /// The playbook file
    pub playbook: String,

    /// The inventory file
    pub inventory: Option<String>,
}

impl Ansible {
    pub fn run(&self, ssh: &mut SshConnection) -> Result<()> {
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

impl Fabricate for Ansible {
    fn run(&self, _ssh: &mut SshConnection) -> Result<()> {
        todo!()
    }
}

impl Prompt for Ansible {
    fn prompt(&mut self, _: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        self.playbook = dialoguer::Input::with_theme(&*theme)
            .with_prompt("Enter the playbook path relative to the current directory")
            .interact()?;

        if !Path::new(&self.playbook).exists() {
            if !dialoguer::Confirm::with_theme(&*theme)
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
