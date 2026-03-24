use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use validator::Validate;

/// A non-root user account to create.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct UnixUser {
    pub username: String,
    pub password: String,
    #[default(false)]
    pub sudo: bool,
}

/// A list of additional user accounts to create.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct UnixUsers(pub Vec<UnixUser>);

impl Prompt for UnixUsers {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::{Confirm, Input, Password};
        let theme = crate::cli::cmd::init::theme();

        loop {
            if !Confirm::with_theme(&theme)
                .with_prompt("Add a user account?")
                .default(false)
                .interact()?
            {
                break;
            }

            let username: String = Input::with_theme(&theme)
                .with_prompt("Username")
                .interact_text()?;

            let password: String = Password::with_theme(&theme)
                .with_prompt(format!("Password for {username}"))
                .with_confirmation("Confirm password", "Passwords do not match")
                .interact()?;

            let sudo: bool = Confirm::with_theme(&theme)
                .with_prompt(format!("Grant sudo to {username}?"))
                .default(false)
                .interact()?;

            self.0.push(UnixUser { username, password, sudo });
        }

        Ok(())
    }
}
