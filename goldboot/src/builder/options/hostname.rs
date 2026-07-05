use std::fmt::Display;

use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use validator::Validate;

/// Sets the network hostname.
#[derive(Clone, Serialize, Deserialize, Debug, SmartDefault)]
pub struct Hostname(#[default("goldboot".to_string())] pub String);

impl Display for Hostname {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Validate for Hostname {
    fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        // RFC 1123: 1-63 alphanumeric characters or hyphens per label, no
        // leading/trailing hyphen
        let valid = !self.0.is_empty()
            && self.0.len() <= 63
            && !self.0.starts_with('-')
            && !self.0.ends_with('-')
            && self
                .0
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-');

        if valid {
            Ok(())
        } else {
            let mut errors = validator::ValidationErrors::new();
            errors.add(
                "hostname",
                validator::ValidationError::new(
                    "Hostname must be 1-63 alphanumeric characters or hyphens and cannot start or end with a hyphen",
                ),
            );
            Err(errors)
        }
    }
}

impl Prompt for Hostname {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let theme = crate::cli::cmd::init::theme();

        self.0 = dialoguer::Input::with_theme(&theme)
            .with_prompt("Enter network hostname")
            .default(self.0.clone())
            .interact()?;

        self.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_hostnames() {
        for hostname in ["goldboot", "a", "web-01", "HOST123"] {
            assert!(
                Hostname(hostname.to_string()).validate().is_ok(),
                "Expected '{hostname}' to be valid"
            );
        }
    }

    #[test]
    fn test_invalid_hostnames() {
        for hostname in [
            "",
            "-web",
            "web-",
            "host name",
            "host_name",
            &"a".repeat(64),
        ] {
            assert!(
                Hostname(hostname.to_string()).validate().is_err(),
                "Expected '{hostname}' to be invalid"
            );
        }
    }
}
