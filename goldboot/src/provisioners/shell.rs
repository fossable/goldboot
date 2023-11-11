use std::error::Error;

use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use validator::Validate;

use crate::ssh::SshConnection;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ShellProvisioners {
    pub commands: Vec<ShellProvisioner>,
}

/// This provisioner runs an inline shell command.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ShellProvisioner {
    /// The inline command to run
    pub command: String,

    /// Overrides the default run order
    pub order: Option<usize>,
}

impl ShellProvisioner {
    /// Create a new shell provisioner with inline command
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
            order: None,
        }
    }

    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running shell provisioner");

        if ssh.exec(&self.command)? != 0 {
            bail!("Provisioner failed");
        }
        Ok(())
    }
}
