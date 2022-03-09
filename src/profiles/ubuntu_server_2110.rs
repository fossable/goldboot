use crate::{
    config::Config,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::QemuBuilder,
    profile::Profile,
};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Validate)]
struct UbuntuServer2110Profile {
    username: String,
    password: String,
    root_password: String,
}

pub fn init(config: &mut Config) {
    config.base = Some(String::from("UbuntuServer2110"));
    config.profile.insert("username".into(), "admin".into());
    config.profile.insert("password".into(), "admin".into());
    config.iso_url = String::from("<ISO URL>");
    config.iso_checksum = Some(String::from("<ISO checksum>"));
}
