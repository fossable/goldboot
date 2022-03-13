use crate::{
    config::Config,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::QemuBuilder,
    profile::Profile,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct UbuntuServerProfile {
    pub version: String,
    pub username: String,
    pub password: String,
    pub root_password: String,
}
