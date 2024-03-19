use std::fmt::Display;

use crate::{
    cli::prompt::{Prompt, PromptNew},
    foundry::Foundry,
};
use anyhow::Result;
use dialoguer::{theme::Theme, Password};
use serde::{Deserialize, Serialize};
use validator::Validate;

// impl UnixAccountProvisioners {
//     /// Get the root user's password
//     pub fn get_root_password(&self) -> Option<String> {
//         self.users
//             .iter()
//             .filter(|u| u.username == "root")
//             .map(|u| u.password)
//             .next()
//     }
// }

/// This provisioner configures a UNIX-like user account.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct UnixAccountProvisioner {
    #[validate(length(max = 64))]
    pub username: String,

    #[validate(length(max = 64))]
    pub password: String,
}

impl Prompt for UnixAccountProvisioner {
    fn prompt(&mut self, _: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        self.password = Password::with_theme(&*theme)
            .with_prompt("Root password")
            .interact()?;

        self.validate()?;
        Ok(())
    }
}

impl Default for UnixAccountProvisioner {
    fn default() -> Self {
        Self {
            username: String::from("root"),
            password: crate::random_password(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RootPassword {
    /// Simple plaintext password
    Plaintext(String),

    /// Take plaintext password from environment variable
    PlaintextEnv(String),
}

impl Default for RootPassword {
    fn default() -> Self {
        Self::Plaintext("root".to_string())
    }
}

impl PromptNew for RootPassword {
    fn prompt_new(_: &Foundry, theme: Box<dyn Theme>) -> Result<Self> {
        Ok(RootPassword::Plaintext(
            Password::with_theme(&*theme)
                .with_prompt("Root password")
                .interact()?,
        ))
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
