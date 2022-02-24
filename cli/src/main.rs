use clap::{Parser, Subcommand};
use anyhow::Result;

pub mod packer;

/// Goldboot CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {

	/// Build gold images
	Build {},

	/// Push gold images
	Push {},
}

pub fn main() -> Result<()> {
    let args = Args::parse();
	Ok(())
}