use anyhow::Result;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use tracing::info;
use validator::Validate;

use crate::builder::ssh::SshConnection;

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

    pub fn run(&self, ssh: &mut SshConnection) -> Result<()> {
        info!("Running shell commands");

        if ssh.exec(&self.command)? != 0 {
            bail!("Shell commands failed");
        }
        Ok(())
    }
}
