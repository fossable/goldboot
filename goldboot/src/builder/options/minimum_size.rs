use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use validator::Validate;

/// Minimum on-disk size of an image element. Images may expand beyond this at deployment time.
#[derive(Clone, Serialize, Deserialize, Debug, SmartDefault)]
pub struct MinimumSize(#[default("16G".to_string())] String);

impl Prompt for MinimumSize {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::Input;
        let theme = crate::cli::cmd::init::theme();

        loop {
            let input: String = Input::with_theme(&theme)
                .with_prompt("Minimum disk size (e.g. 16G, 512M)")
                .default(self.0.clone())
                .interact_text()?;

            let candidate = MinimumSize(input);
            match candidate.validate() {
                Ok(_) => {
                    *self = candidate;
                    return Ok(());
                }
                Err(e) => eprintln!("Invalid size: {e}"),
            }
        }
    }
}

impl Validate for MinimumSize {
    fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        // Try to parse the size string using byte-unit
        match self.0.parse::<Byte>() {
            Ok(byte) => {
                // Ensure the size is greater than zero
                if byte.as_u64() == 0 {
                    let mut errors = validator::ValidationErrors::new();
                    errors.add(
                        "size",
                        validator::ValidationError::new("Size must be greater than zero"),
                    );
                    return Err(errors);
                }
                Ok(())
            }
            Err(_) => {
                let mut errors = validator::ValidationErrors::new();
                errors.add(
                    "size",
                    validator::ValidationError::new("Invalid size format. Expected format: number followed by unit (e.g., '16G', '512M', '1T')"),
                );
                Err(errors)
            }
        }
    }
}

impl From<MinimumSize> for u64 {
    fn from(val: MinimumSize) -> Self {
        // Assume MinimumSize was validated previously
        val.0
            .parse::<Byte>()
            .expect("MinimumSize was not validated")
            .as_u64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sizes() {
        // Test various valid size formats
        let sizes = vec![
            "16G", "16GB", "512M", "512MB", "1T", "1TB", "2048K", "2048KB", "1024", "1 GB",
            "1.5 GB", "100 MiB", "16GiB",
        ];

        for size_str in sizes {
            let size = MinimumSize(size_str.to_string());
            assert!(
                size.validate().is_ok(),
                "Expected '{}' to be valid",
                size_str
            );
        }
    }

    #[test]
    fn test_invalid_sizes() {
        // Test various invalid size formats
        let invalid_sizes = vec![
            "",            // Empty string
            "abc",         // No numbers
            "G16",         // Unit before number
            "16X",         // Invalid unit
            "hello world", // Completely invalid
        ];

        for size_str in invalid_sizes {
            let size = MinimumSize(size_str.to_string());
            assert!(
                size.validate().is_err(),
                "Expected '{}' to be invalid",
                size_str
            );
        }
    }

    #[test]
    fn test_zero_size() {
        // Test that zero size is rejected
        let size = MinimumSize("0".to_string());
        assert!(size.validate().is_err(), "Expected zero size to be invalid");

        let size = MinimumSize("0GB".to_string());
        assert!(size.validate().is_err(), "Expected zero size to be invalid");
    }
}
