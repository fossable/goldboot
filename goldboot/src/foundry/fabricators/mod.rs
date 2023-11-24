//! This module contains various common provisioners which may be included in
//! image templates. Templates may also specify their own specialized
//! provisioners for specific tasks.

use crate::ssh::SshConnection;
use std::error::Error;

pub mod ansible;
pub mod exe;
pub mod shell;

/// A `Fabricator` performs some custom operation on an image at the very end of
/// the casting process. When the various options in a `Mold` are not sufficient,
/// fabricators can be used to compensate.
pub trait Fabricator {
    fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>>;
}
