use log::{debug, info};
use std::error::Error;
use std::io::Cursor;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;

pub struct SshConnection {
    pub username: String,
    pub password: String,
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
            session,
        })
    }

    pub fn shutdown(&self, command: &str) -> Result<(), Box<dyn Error>> {
        info!("Sending shutdown command");
        let mut channel = self.session.channel_session()?;
        channel.exec(command)?;
        Ok(())
    }

    pub fn upload_exec(&self, source: Vec<u8>, env: Vec<String>) -> Result<(), Box<dyn Error>> {
        self.upload(&mut Cursor::new(source), "tmp.script")?;
        self.exec("tmp.script")?;
        self.exec("rm -f tmp.script")?;
        Ok(())
    }

    pub fn upload(&self, source: &mut impl Read, dest: &str) -> Result<(), Box<dyn Error>> {
        let mut remote_file = self.session.scp_send(Path::new(dest), 0o700, 10, None)?;
        std::io::copy(source, &mut remote_file)?;

        remote_file.send_eof()?;
        remote_file.wait_eof()?;
        remote_file.close()?;
        remote_file.wait_close()?;

        Ok(())
    }

    pub fn exec(&self, cmdline: &str) -> Result<i32, Box<dyn Error>> {
        let mut channel = self.session.channel_session()?;
        channel.exec(cmdline)?;
        //let mut s = String::new();
        //channel.read_to_string(&mut s).unwrap();
        channel.wait_close()?;
        Ok(channel.exit_status()?)
    }
}
