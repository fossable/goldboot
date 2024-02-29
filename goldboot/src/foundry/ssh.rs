use anyhow::bail;
use anyhow::Result;
use goldboot_image::ImageArch;
use rand::Rng;
use ssh2::Session;
use ssh_key::Algorithm;
use ssh_key::LineEnding;
use ssh_key::PrivateKey;
use std::io::Read;
use std::path::PathBuf;
use std::thread::sleep;
use std::{
    io::{BufRead, BufReader, Cursor},
    net::TcpStream,
    path::Path,
    time::Duration,
};
use tracing::{debug, info};

use super::qemu::OsCategory;

/// Generate a new random SSH keypair
pub fn generate_key(directory: &Path) -> Result<PathBuf> {
    let key_name: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    let key_path = directory.join(key_name);

    let private_key = PrivateKey::random(&mut rand::thread_rng(), Algorithm::Ed25519)?;
    std::fs::write(&key_path, private_key.to_openssh(LineEnding::LF)?)?;
    std::fs::write(
        &key_path.with_extension("pub"),
        private_key.public_key().to_openssh()?,
    )?;

    Ok(key_path)
}

/// Download and extract sshdog.
pub fn download_sshdog(arch: ImageArch, os_category: OsCategory) -> Result<Vec<u8>> {
    let url = format!(
        "https://github.com/fossable/sshdog/releases/download/v0.2.1/sshdog_0.2.1_{}_{}.tar.gz",
        os_category, arch,
    )
    .to_lowercase();

    debug!(url, "Downloading sshdog");
    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        bail!("Failed to download");
    }
    let mut uncompressed = flate2::read::GzDecoder::new(response);
    let mut archive = tar::Archive::new(uncompressed);

    for entry in archive.entries()? {
        let mut entry = entry?;

        if entry.path()?.to_string_lossy().to_string() == "sshdog" {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            return Ok(content);
        }
    }

    bail!("Executable not found in archive");
}

/// Represents an SSH session to a running VM.
pub struct SshConnection {
    pub username: String,
    pub private_key: PathBuf,
    pub port: u16,
    pub session: ssh2::Session,
}

impl SshConnection {
    pub fn new(username: &str, private_key: &PathBuf, port: u16) -> Result<SshConnection> {
        let mut i = 0;
        Ok(loop {
            i += 1;
            debug!("Trying SSH: {}@localhost:{}", username, port);

            match Self::connect(username, private_key, port) {
                Ok(session) => {
                    break SshConnection {
                        username: username.to_string(),
                        private_key: private_key.clone(),
                        port,
                        session,
                    }
                }
                Err(error) => debug!("{}", error),
            };

            if i > 25 {
                bail!("Maximum iterations reached");
            }

            std::thread::sleep(Duration::from_secs(5));
        })
    }

    fn connect(username: &str, private_key: &PathBuf, port: u16) -> Result<Session> {
        let mut session = ssh2::Session::new()?;
        session.set_tcp_stream(TcpStream::connect(format!("127.0.0.1:{port}"))?);

        session.handshake()?;
        session.userauth_pubkey_file(username, None, private_key, None)?;
        info!("Established SSH connection");
        Ok(session)
    }

    /// Send the shutdown command to the VM.
    pub fn shutdown(&self, command: &str) -> Result<()> {
        info!("Sending shutdown command");
        let mut channel = self.session.channel_session()?;
        channel.exec(command)?;
        Ok(())
    }

    pub fn upload_exec(&mut self, source: &[u8], env: Vec<(&str, &str)>) -> Result<i32> {
        let id: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let path = format!("/tmp/gb_{id}");

        self.upload(source, &path)?;
        let exit = self.exec_env(&path, env)?;

        // Attempt to cleanup, but don't fail if we can't
        _ = self.exec(&format!("rm -f {path}"));
        Ok(exit)
    }

    pub fn upload(&self, source: &[u8], dest: &str) -> Result<()> {
        debug!(bytes = source.len(), dest, "Uploading file with scp");
        let mut channel =
            self.session
                .scp_send(Path::new(dest), 0o700, source.len().try_into()?, None)?;
        std::io::copy(&mut Cursor::new(source), &mut channel)?;

        channel.send_eof()?;
        channel.wait_eof()?;
        channel.close()?;
        channel.wait_close()?;

        Ok(())
    }

    /// Run a command on the VM with the given environment.
    pub fn exec_env(&mut self, cmdline: &str, env: Vec<(&str, &str)>) -> Result<i32> {
        debug!(cmdline, environment = ?env, "Executing command over ssh");

        let mut channel = self.session.channel_session()?;

        // Set environment
        for (var, val) in env {
            channel.setenv(&var, &val)?;
        }

        channel.exec(cmdline)?;

        let mut stdout = BufReader::new(channel.stderr());

        loop {
            let mut line = String::new();
            match stdout.read_line(&mut line) {
                Ok(0) => break,
                // TODO part of some span like goldboot::foundry::fabricator::exe
                Ok(_) => debug!(
                    "(fabricator) {}",
                    line.strip_suffix("\r\n")
                        .or(line.strip_suffix("\n"))
                        .unwrap_or(&line)
                ),
                Err(_) => {
                    // The VM is probably rebooting, wait for SSH to come back up
                    info!("SSH disconnected; waiting for it to come back");
                    std::thread::sleep(Duration::from_secs(10));
                    for _ in 0..5 {
                        match Self::connect(&self.username, &self.private_key, self.port) {
                            Ok(session) => {
                                self.session = session;
                                return Ok(0);
                            }
                            Err(_) => std::thread::sleep(Duration::from_secs(50)),
                        }
                    }
                    bail!("SSH did not come back in a reasonable amount of time");
                }
            }
        }

        channel.wait_close()?;
        let exit = channel.exit_status()?;
        debug!("Exit code: {}", exit);
        Ok(exit)
    }

    /// Run a command on the VM.
    pub fn exec(&mut self, cmdline: &str) -> Result<i32> {
        self.exec_env(cmdline, Vec::new())
    }
}
