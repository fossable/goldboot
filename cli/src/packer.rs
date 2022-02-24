use std::process::Command;
use std::fs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct PackerTemplate {
	builders: Vec<QemuBuilder>,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct QemuBuilder {
	pub r#type: String,
}

impl PackerTemplate {
	pub fn build(&self) -> Result<Box<Command>> {

		// Acquire temporary directory for the build
		let context = tempfile::tempdir().unwrap();

		// Copy build context
		// TODO

		// Generate the packer template
		fs::write(context.path().join("packer.json"), serde_json::to_string(&self).unwrap()).unwrap();

		Ok(Command::new("packer")
			.arg("build")
			.arg("packer.json"))
	}
}