use anyhow::Result;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use crate::builder::os::{OsConfig, os_config_from_ron};

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

    /// Return the directory containing this config file.
    pub fn context_dir(&self) -> PathBuf {
        self.0
            .parent()
            .expect("config path has no parent")
            .to_path_buf()
    }

    /// Read and deserialize the Ron configuration file.
    pub fn load(&self) -> Result<Vec<OsConfig>> {
        let content = std::fs::read_to_string(&self.0)?;
        let trimmed = content.trim();

        // Check if it's a list (starts with '[')
        if trimmed.starts_with('[') {
            // Split the list into individual elements and parse each
            // Use ron::Value to split the array, then re-serialize each element
            let values: Vec<ron::Value> = ron::from_str(trimmed)?;
            let mut result = Vec::with_capacity(values.len());
            for value in values {
                let s = ron::to_string(&value)?;
                result.push(os_config_from_ron(&s)?);
            }
            return Ok(result);
        }

        // Single OS config
        Ok(vec![os_config_from_ron(trimmed)?])
    }

    /// Write a new Ron configuration file.
    pub fn write(&self, elements: &Vec<OsConfig>) -> Result<()> {
        let ron_config = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .enumerate_arrays(false)
            .compact_arrays(false);

        let ron_content = if elements.len() == 1 {
            elements[0].0.serialize_ron(&ron_config)?
        } else {
            // Serialize each element and wrap in a list
            let parts: Vec<String> = elements
                .iter()
                .map(|e| e.0.serialize_ron(&ron_config))
                .collect::<anyhow::Result<Vec<_>>>()?;
            format!("[\n{}\n]", parts.join(",\n"))
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
