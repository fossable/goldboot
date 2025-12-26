use anyhow::Result;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use crate::builder::os::Os;

/// Represents a builder configuration file path
#[derive(Clone, Debug)]
pub struct ConfigPath(PathBuf);

impl Default for ConfigPath {
    fn default() -> Self {
        ConfigPath(PathBuf::from("./goldboot.ron"))
    }
}

impl ConfigPath {
    /// Check for a builder configuration file in the given directory.
    pub fn from_dir(path: impl AsRef<Path>) -> Option<ConfigPath> {
        let path = path.as_ref();

        if path.join("goldboot.ron").exists() {
            return Some(ConfigPath(path.join("goldboot.ron")));
        }

        None
    }

    /// Read and deserialize the Ron configuration file.
    pub fn load(&self) -> Result<Vec<Os>> {
        let content = std::fs::read_to_string(&self.0)?;

        // Try to deserialize as Vec<Os> first
        if let Ok(os_vec) = ron::from_str::<Vec<Os>>(&content) {
            return Ok(os_vec);
        }

        // Try single Os
        if let Ok(os) = ron::from_str::<Os>(&content) {
            return Ok(vec![os]);
        }

        Err(anyhow::anyhow!(
            "Failed to parse Ron config. Expected either a single OS configuration or a list of OS configurations."
        ))
    }

    /// Write a new Ron configuration file.
    pub fn write(&self, elements: &Vec<Os>) -> Result<()> {
        let ron_config = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .enumerate_arrays(false)
            .compact_arrays(false);

        let ron_content = if elements.len() == 1 {
            ron::ser::to_string_pretty(&elements[0], ron_config)?
        } else {
            ron::ser::to_string_pretty(elements, ron_config)?
        };

        std::fs::write(&self.0, ron_content.as_bytes())?;
        Ok(())
    }
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.to_string_lossy().fmt(f)
    }
}
