#![feature(seek_stream_len)]
#![feature(let_chains)]

use crate::build::BuildConfig;
use log::{debug, info};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{default::Default, error::Error, net::TcpListener, process::Command};
use strum::{Display, EnumIter};
use validator::Validate;

pub mod build;
pub mod cache;
pub mod cmd;
pub mod http;
pub mod image;
pub mod library;
pub mod progress;
pub mod provisioners;
pub mod qcow;
pub mod qemu;
pub mod registry;
pub mod ssh;
pub mod templates;
pub mod vnc;

/// Find a random open TCP port in the given range.
pub fn find_open_port(lower: u16, upper: u16) -> u16 {
	let mut rand = rand::thread_rng();

	loop {
		let port = rand.gen_range(lower..upper);
		match TcpListener::bind(format!("0.0.0.0:{port}")) {
			Ok(_) => break port,
			Err(_) => continue,
		}
	}
}

/// Generate a random password
pub fn random_password() -> String {
	// TODO check for a dictionary to generate something memorable

	// Fallback to random letters and numbers
	rand::thread_rng()
		.sample_iter(&rand::distributions::Alphanumeric)
		.take(12)
		.map(char::from)
		.collect()
}

pub fn is_interactive() -> bool {
	!std::env::var("CI").is_ok()
}

/// Represents a system architecture.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, PartialEq, Eq, EnumIter, Display)]
#[serde(tag = "arch")]
#[allow(non_camel_case_types)]
pub enum Architecture {
	#[default]
	amd64,
	arm64,
	i386,
	mips,
	s390x,
}

impl TryFrom<String> for Architecture {
	type Error = Box<dyn Error>;
	fn try_from(s: String) -> Result<Self, Self::Error> {
		match s.to_lowercase().as_str() {
			"amd64" => Ok(Architecture::amd64),
			"x86_64" => Ok(Architecture::amd64),
			"arm64" => Ok(Architecture::arm64),
			"aarch64" => Ok(Architecture::arm64),
			"i386" => Ok(Architecture::i386),
			_ => bail!("Unknown architecture"),
		}
	}
}

pub trait PromptMut {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>>;
}

pub trait Prompt {
	fn prompt(
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<Self, Box<dyn Error>> where Self: Sized;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_open_port() {
		let port = find_open_port(9000, 9999);

		assert!(port < 9999);
		assert!(port >= 9000);
	}
}
