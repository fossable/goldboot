use std::error::Error;

pub struct SshConnection {}

impl SshConnection {
    pub fn run(&self, command: &str) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
