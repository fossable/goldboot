pub mod start;
pub mod user;

use std::path::PathBuf;

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Start the registry HTTP server
    Start {
        /// Path to the registry config file (TOML).
        #[clap(long, default_value = "/etc/goldboot-registry/config.toml")]
        config: PathBuf,
    },

    /// Manage user accounts (stored in the config file)
    User {
        #[clap(subcommand)]
        command: UserCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum UserCommands {
    /// Add a user (prompts for password)
    Add {
        /// Path to the registry config file
        #[clap(long, default_value = "/etc/goldboot-registry/config.toml")]
        config: PathBuf,
        /// Username
        #[clap(index = 1)]
        username: String,
        /// Grant pull permission
        #[clap(long, default_value_t = true)]
        pull: bool,
        /// Grant push permission
        #[clap(long, default_value_t = false)]
        push: bool,
    },

    /// Remove a user
    Remove {
        #[clap(long, default_value = "/etc/goldboot-registry/config.toml")]
        config: PathBuf,
        #[clap(index = 1)]
        username: String,
    },

    /// List users
    List {
        #[clap(long, default_value = "/etc/goldboot-registry/config.toml")]
        config: PathBuf,
    },

    /// Hash a password and print the PHC string (useful for NixOS / Ansible /
    /// any system that needs to inject the hash via file rather than running
    /// `user add` interactively).
    Hash {
        /// Read password from stdin instead of prompting interactively
        #[clap(long)]
        stdin: bool,
    },
}
