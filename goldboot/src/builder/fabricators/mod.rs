//! This module contains various common provisioners which may be included in
//! image templates. Templates may also specify their own specialized
//! provisioners for specific tasks.

use crate::builder::ssh::SshConnection;
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod ansible;
pub mod exe;
pub mod shell;

/// A `Fabricator` performs some custom operation on an image at the very end of
/// the build process.
pub trait Fabricate {
    fn run(&self, ssh: &mut SshConnection) -> Result<()>;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Fabricator {
    Ansible(ansible::Ansible),
}

impl Fabricate for Fabricator {
    fn run(&self, ssh: &mut SshConnection) -> Result<()> {
        match self {
            Fabricator::Ansible(inner) => inner.run(ssh),
        }
    }
}
