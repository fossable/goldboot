use std::error::Error;

use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use validator::Validate;

use crate::foundry::ssh::SshConnection;

/// Runs an inline shell command.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ShellFabricator {
    /// The inline command to run
    pub command: String,
}

impl ShellFabricator {
    /// Create a new shell fabricator with inline command
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
        }
    }

    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running shell commands");

        if ssh.exec(&self.command)? != 0 {
            bail!("Shell commands failed");
        }
        Ok(())
    }
}
