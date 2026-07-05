//! Shared `ansible-playbook` invocation for the ansible pre/post steps.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};
use tracing::info;

use crate::builder::ssh::SshConnection;

/// How ansible reaches its target: `Local` runs the playbook on the host
/// itself; `Ssh` runs it against the VM's forwarded SSH port.
pub enum Connection<'a> {
    Local,
    Ssh(&'a SshConnection),
}

/// Run `ansible-playbook` from `cwd` (the effective context directory), so
/// playbook and vars-file paths in the config resolve relative to the
/// context dir.
pub fn run_playbook(
    cwd: &Path,
    playbook: &str,
    inventory: Option<&str>,
    vars_files: &Option<Vec<String>>,
    extra_vars: &Option<HashMap<String, String>>,
    connection: Connection<'_>,
) -> Result<()> {
    info!(%playbook, "Running ansible playbook");

    let mut cmd = playbook_command(cwd, playbook, inventory, vars_files, extra_vars, connection)?;

    let status = cmd
        .status()
        .map_err(|e| anyhow::anyhow!("failed to launch ansible-playbook: {e}"))?;
    if !status.success() {
        bail!("ansible-playbook exited with {status}");
    }
    Ok(())
}

fn playbook_command(
    cwd: &Path,
    playbook: &str,
    inventory: Option<&str>,
    vars_files: &Option<Vec<String>>,
    extra_vars: &Option<HashMap<String, String>>,
    connection: Connection<'_>,
) -> Result<Command> {
    let mut cmd = Command::new("ansible-playbook");
    cmd.current_dir(cwd);

    match connection {
        Connection::Local => {
            cmd.arg("-i").arg("localhost,").arg("-c").arg("local");
        }
        Connection::Ssh(ssh) => {
            // Single-host inventory pointing at the VM's forwarded SSH port;
            // playbooks should target `hosts: all`.
            cmd.arg("-i")
                .arg(inventory.unwrap_or("127.0.0.1,"))
                .arg("--ssh-common-args")
                .arg("-oStrictHostKeyChecking=no");

            let mut vars = serde_json::Map::new();
            vars.insert("ansible_port".into(), ssh.port.into());
            vars.insert("ansible_user".into(), ssh.username.clone().into());
            vars.insert(
                "ansible_ssh_private_key_file".into(),
                ssh.private_key.display().to_string().into(),
            );
            vars.insert("ansible_connection".into(), "ssh".into());
            cmd.arg("-e")
                .arg(serde_json::Value::Object(vars).to_string());
        }
    }

    if let Some(files) = vars_files {
        for f in files {
            cmd.arg("-e").arg(format!("@{f}"));
        }
    }
    if let Some(vars) = extra_vars {
        // JSON form so values containing spaces or quotes survive intact
        cmd.arg("-e").arg(serde_json::to_string(vars)?);
    }
    cmd.arg(playbook);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn args(cmd: &Command) -> Vec<&OsStr> {
        cmd.get_args().collect()
    }

    #[test]
    fn local_command_args() -> Result<()> {
        let extra_vars = Some(HashMap::from([(
            "hostname".to_string(),
            "with space".to_string(),
        )]));
        let cmd = playbook_command(
            Path::new("/tmp"),
            "render.yml",
            None,
            &Some(vec!["vars.yml".to_string()]),
            &extra_vars,
            Connection::Local,
        )?;

        assert_eq!(
            args(&cmd),
            vec![
                "-i",
                "localhost,",
                "-c",
                "local",
                "-e",
                "@vars.yml",
                "-e",
                r#"{"hostname":"with space"}"#,
                "render.yml",
            ]
        );
        assert_eq!(cmd.get_current_dir(), Some(Path::new("/tmp")));
        Ok(())
    }
}
