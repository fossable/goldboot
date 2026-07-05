use std::fmt::Display;

use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use dialoguer::Password;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// Sets the root account's password.
#[derive(Clone, Serialize, Deserialize, Debug, SmartDefault)]
#[serde(rename_all = "snake_case")]
pub enum RootPassword {
    /// Simple plaintext password
    #[default]
    Plaintext(#[default("root".to_string())] String),

    /// Take plaintext password from environment variable
    PlaintextEnv(String),
}

impl Prompt for RootPassword {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let theme = crate::cli::cmd::init::theme();

        *self = RootPassword::Plaintext(
            Password::with_theme(&theme)
                .with_prompt("Root password")
                .interact()?,
        );
        Ok(())
    }
}

impl Display for RootPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                RootPassword::Plaintext(password) => format!("plain:{password}"),
                RootPassword::PlaintextEnv(name) => format!(
                    "plain:{}",
                    std::env::var(name).expect("environment variable not found")
                ),
            }
        )
    }
}
