//! This module contains various common provisioners which may be included in
//! image templates. Templates may also specify their own specialized
//! provisioners for specific tasks.

use crate::builder::ssh::SshConnection;
use ansible::Ansible;
use anyhow::Result;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

pub mod ansible;
pub mod exe;
pub mod shell;

/// A `Fabricator` performs some custom operation on an image at the very end of
/// the build process.
#[enum_dispatch(Fabricator)]
pub trait Fabricate {
    fn run(&self, ssh: &mut SshConnection) -> Result<()>;
}

#[enum_dispatch]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Fabricator {
    Ansible,
}
