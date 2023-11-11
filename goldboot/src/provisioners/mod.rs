//! This module contains various common provisioners which may be included in
//! image templates. Templates may also specify their own specialized
//! provisioners for specific tasks.
//!
//! A provisioner is simply an operation to be performed on an image.

use crate::ssh::SshConnection;
use std::error::Error;

pub mod ansible;
pub mod exe;
pub mod hostname;
pub mod luks;
pub mod shell;
pub mod timezone;
pub mod unix_account;

pub trait Provisioner {
    fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>>;
}
