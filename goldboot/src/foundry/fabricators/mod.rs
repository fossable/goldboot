//! This module contains various common provisioners which may be included in
//! image templates. Templates may also specify their own specialized
//! provisioners for specific tasks.

use crate::foundry::ssh::SshConnection;
use ansible::Ansible;
use anyhow::Result;
use enum_dispatch::enum_dispatch;

pub mod ansible;
pub mod exe;
pub mod shell;

/// A `Fabricator` performs some custom operation on an image at the very end of
/// the casting process. When the various options in a `Mold` are not sufficient,
/// fabricators can be used to compensate.
#[enum_dispatch(Fabricator)]
pub trait Fabricate {
    fn run(&self, ssh: &mut SshConnection) -> Result<()>;
}

#[enum_dispatch]
pub enum Fabricator {
    Ansible,
}
