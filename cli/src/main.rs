use std::path::Path;
use clap::{Parser, Subcommand};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use validator::Validate;
use std::fs;
use std::env;

pub mod packer;
pub mod qemu;

/// Goldboot CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {

	/// Build image
	Build {},

	Registry {},

	Images {},

	/// Write image to an unmounted disk
	Write {},

	Init {
		profile: String,
	},
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
struct Config {

	pub name: String,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,

	pub base: String,

	pub provisioners: Vec<Provisioner>,

	pub qemu: qemu::QemuConfig,
}

/// A generic provisioner
#[derive(Clone, Serialize, Deserialize, Validate)]
struct Provisioner {

	pub r#type: String,

	#[serde(flatten)]
	pub ansible: AnsibleProvisioner,

	#[serde(flatten)]
	pub shell: ShellProvisioner,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
struct AnsibleProvisioner {

	pub playbook: String,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
struct ShellProvisioner {

	pub script: String,

	pub inline: Vec<String>,
}

fn load_config() -> Result<Config> {

	// Read config from working directory
	let config: Config = serde_json::from_slice(&fs::read("goldboot.json").unwrap()).unwrap();

	// TODO add base config
	Ok(config)
}

fn build(cl: CommandLine) -> Result<()> {
	Ok(())
}

fn init(cl: CommandLine) -> Result<()> {

	if Path::new("goldboot.json").exists() {
		bail!("This directory has already been initialized");
	}

	let mut config = Config::default();

	// Set name equal to directory name
	if let Some(name) = env::current_dir().unwrap().file_name() {
		config.name = name.to_str().unwrap().to_string();
	}

	// Generate QEMU flags for this hardware

	// Write out the config
	fs::write(Path::new("goldboot.json"), serde_json::to_string(&config).unwrap()).unwrap();

	Ok(())
}

pub fn main() -> Result<()> {
    let cl = CommandLine::parse();

    match &cl.command {
    	Commands::Build {} => build(cl),
    	Commands::Init {profile} => init(cl),
    }
}