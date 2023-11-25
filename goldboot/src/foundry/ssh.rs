use log::{debug, info};
use simple_error::bail;
use std::{
    error::Error,
    io::{BufRead, BufReader, Cursor},
    net::TcpStream,
    path::Path,
    time::Duration,
};

/// Represents an SSH session to a running VM.
pub struct SshConnection {
    pub username: String,
    pub password: String,
    pub port: u16,
    pub session: ssh2::Session,
}

impl SshConnection {
    pub fn new(port: u16, username: &str, password: &str) -> Result<SshConnection, Box<dyn Error>> {
        debug!("Trying SSH: {}@localhost:{}", username, port);

        let mut session = ssh2::Session::new()?;
        session.set_tcp_stream(TcpStream::connect(format!("127.0.0.1:{port}"))?);

        session.handshake()?;
        session.userauth_password(username, password)?;

        info!("Established SSH connection");
        Ok(SshConnection {
            username: username.to_string(),
            password: password.to_string(),
            port,
            session,
        })
    }

    /// Send the shutdown command to the VM.
    pub fn shutdown(&self, command: &str) -> Result<(), Box<dyn Error>> {
        info!("Sending shutdown command");
        let mut channel = self.session.channel_session()?;
        channel.exec(command)?;
        Ok(())
    }

    pub fn upload_exec(
        &mut self,
        source: &[u8],
        env: Vec<(&str, &str)>,
    ) -> Result<i32, Box<dyn Error>> {
        self.upload(source, "/tmp/tmp.script")?;
        let exit = self.exec_env("/tmp/tmp.script", env)?;
        self.exec("rm -f /tmp/tmp.script")?;
        Ok(exit)
    }

    pub fn upload(&self, source: &[u8], dest: &str) -> Result<(), Box<dyn Error>> {
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
    pub fn exec_env(
        &mut self,
        cmdline: &str,
        env: Vec<(&str, &str)>,
    ) -> Result<i32, Box<dyn Error>> {
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
                        match SshConnection::new(self.port, &self.username, &self.password) {
                            Ok(ssh) => {
                                // Steal the session
                                self.session = ssh.session;
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
    pub fn exec(&mut self, cmdline: &str) -> Result<i32, Box<dyn Error>> {
        self.exec_env(cmdline, Vec::new())
    }
}
