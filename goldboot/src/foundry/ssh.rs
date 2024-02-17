use anyhow::bail;
use anyhow::Result;
use ssh2::Session;
use std::path::PathBuf;
use std::{
    io::{BufRead, BufReader, Cursor},
    net::TcpStream,
    path::Path,
    time::Duration,
};
use tracing::{debug, info};

/// Generate a new random SSH private key
pub fn generate_private_key(directory: &Path) -> PathBuf {
    todo!()
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
        self.upload(source, "/tmp/tmp.script")?;
        let exit = self.exec_env("/tmp/tmp.script", env)?;
        self.exec("rm -f /tmp/tmp.script")?;
        Ok(exit)
    }

    pub fn upload(&self, source: &[u8], dest: &str) -> Result<()> {
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
        debug!("Executing command: '{}'", cmdline);

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
