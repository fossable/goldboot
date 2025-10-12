use crate::{builder::Builder, cli::prompt::Prompt};
use anyhow::Result;
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Absolute size of an image element.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Size(String);

impl Default for Size {
    fn default() -> Self {
        Self("16G".to_string())
    }
}

impl Prompt for Size {
    fn prompt(&mut self, builder: &Builder) -> Result<()> {
        todo!()
    }
}

impl Validate for Size {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sizes() {
        // Test various valid size formats
        let sizes = vec![
            "16G",
            "16GB",
            "512M",
            "512MB",
            "1T",
            "1TB",
            "2048K",
            "2048KB",
            "1024",
            "1 GB",
            "1.5 GB",
            "100 MiB",
            "16GiB",
        ];

        for size_str in sizes {
            let size = Size(size_str.to_string());
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
            "",           // Empty string
            "abc",        // No numbers
            "G16",        // Unit before number
            "16X",        // Invalid unit
            "hello world", // Completely invalid
        ];

        for size_str in invalid_sizes {
            let size = Size(size_str.to_string());
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
        let size = Size("0".to_string());
        assert!(
            size.validate().is_err(),
            "Expected zero size to be invalid"
        );

        let size = Size("0GB".to_string());
        assert!(
            size.validate().is_err(),
            "Expected zero size to be invalid"
        );
    }

    #[test]
    fn test_default_size() {
        // Test that the default size is valid
        let size = Size::default();
        assert!(
            size.validate().is_ok(),
            "Expected default size '{}' to be valid",
            size.0
        );
    }

    #[test]
    fn test_large_sizes() {
        // Test large sizes
        let sizes = vec![
            "1000T",
            "1000TB",
            "1PB",
            "100000GB",
        ];

        for size_str in sizes {
            let size = Size(size_str.to_string());
            assert!(
                size.validate().is_ok(),
                "Expected large size '{}' to be valid",
                size_str
            );
        }
    }

    #[test]
    fn test_bytes_only() {
        // Test sizes specified in bytes only
        let size = Size("1073741824".to_string()); // 1GB in bytes
        assert!(
            size.validate().is_ok(),
            "Expected bytes-only size to be valid"
        );
    }

    #[test]
    fn test_binary_vs_decimal_units() {
        // Test both binary (GiB) and decimal (GB) units
        let size_binary = Size("16GiB".to_string());
        assert!(
            size_binary.validate().is_ok(),
            "Expected binary unit GiB to be valid"
        );

        let size_decimal = Size("16GB".to_string());
        assert!(
            size_decimal.validate().is_ok(),
            "Expected decimal unit GB to be valid"
        );
    }
}
