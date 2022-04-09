use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
pub struct UbuntuServerProfile {
    pub version: String,
    pub username: String,
    pub password: String,
    pub root_password: String,
}
