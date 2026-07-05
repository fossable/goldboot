//! Build steps that run around the image build. A `PreStep` runs on the host
//! before the VM boots and may freely modify the build's effective context
//! directory (an ephemeral copy of the directory containing `goldboot.ron`).
//! A `PostStep` runs against the booted VM over SSH after the install
//! completes.

use std::collections::HashMap;
use std::path::Path;

use crate::builder::Builder;
use crate::builder::ssh::SshConnection;
use crate::cli::prompt::Prompt;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use tracing::info;

pub mod ansible;

/// A `PreStep` runs on the host before the VM boots. It receives the build's
/// effective context directory and may modify it in place, e.g. to render
/// templated config files.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum PreStep {
    /// Runs an Ansible playbook locally against the effective context
    /// directory. All paths are relative to the context dir.
    AnsibleLocal {
        /// The playbook file
        playbook: String,

        /// Vars files, each passed as `-e @<file>` to ansible-playbook
        #[serde(default)]
        vars_files: Option<Vec<String>>,

        /// Extra variables, passed to ansible-playbook as JSON via `-e`
        #[serde(default)]
        extra_vars: Option<HashMap<String, String>>,
    },
}

impl PreStep {
    pub fn run(&self, context_dir: &Path) -> Result<()> {
        match self {
            Self::AnsibleLocal {
                playbook,
                vars_files,
                extra_vars,
            } => ansible::run_playbook(
                context_dir,
                playbook,
                None,
                vars_files,
                extra_vars,
                ansible::Connection::Local,
            ),
        }
    }
}

/// A `PostStep` runs against the booted VM over SSH after the install
/// completes. All paths are relative to the effective context directory.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum PostStep {
    /// Runs an Ansible playbook on the VM over SSH. The playbook should
    /// target `hosts: all`.
    Ansible {
        /// The playbook file
        playbook: String,

        /// Inventory file passed as `-i`; defaults to the VM's forwarded
        /// SSH port on 127.0.0.1
        #[serde(default)]
        inventory: Option<String>,

        /// Vars files, each passed as `-e @<file>` to ansible-playbook
        #[serde(default)]
        vars_files: Option<Vec<String>>,

        /// Extra variables, passed to ansible-playbook as JSON via `-e`
        #[serde(default)]
        extra_vars: Option<HashMap<String, String>>,
    },
    /// Uploads an executable from the host and runs it on the VM.
    HostExecutable {
        /// The path to the executable
        path: String,
    },
    /// Runs an inline shell command on the VM.
    Shell {
        /// The command to run
        command: String,
    },
}

impl PostStep {
    pub fn run(&self, ssh: &mut SshConnection, context_dir: &Path) -> Result<()> {
        match self {
            Self::Ansible {
                playbook,
                inventory,
                vars_files,
                extra_vars,
            } => ansible::run_playbook(
                context_dir,
                playbook,
                inventory.as_deref(),
                vars_files,
                extra_vars,
                ansible::Connection::Ssh(ssh),
            ),
            Self::HostExecutable { path } => {
                info!(%path, "Running executable post-step");
                if ssh.upload_exec(&std::fs::read(context_dir.join(path))?, vec![])? != 0 {
                    bail!("Executable failed");
                }
                Ok(())
            }
            Self::Shell { command } => {
                info!("Running shell post-step");
                if ssh.exec(command)? != 0 {
                    bail!("Shell command failed");
                }
                Ok(())
            }
        }
    }
}

impl Prompt for Vec<PreStep> {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

impl Prompt for Vec<PostStep> {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_example_config_steps() -> Result<()> {
        let ron = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../examples/nixos/goldboot.ron"
        ))?;
        let config = crate::builder::os::os_config_from_ron(&ron)?;

        assert!(matches!(
            config.0.pre_steps(),
            [PreStep::AnsibleLocal {
                playbook,
                vars_files: None,
                extra_vars: Some(_),
            }] if playbook == "render-config.yml"
        ));
        assert!(matches!(config.0.post_steps(), [PostStep::Shell { .. }]));
        Ok(())
    }
}
