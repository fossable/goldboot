use anyhow::Result;
use clap::{ValueEnum, builder::PossibleValue};
#[cfg(feature = "config-python")]
use pyo3::{
    ffi::c_str,
    prelude::*,
    types::{IntoPyDict, PyModule},
};
use std::{
    ffi::CString,
    fmt::Display,
    path::{Path, PathBuf},
    sync::OnceLock,
    thread,
    time::SystemTime,
};
use strum::EnumIter;

use crate::foundry::Foundry;

/// Represents a foundry configuration file. This mainly helps sort out the various
/// supported config formats.
#[derive(Clone, Debug, EnumIter)]
pub enum ConfigPath {
    #[cfg(feature = "config-json")]
    Json(PathBuf),
    #[cfg(feature = "config-python")]
    Python(PathBuf),
    #[cfg(feature = "config-ron")]
    Ron(PathBuf),
    #[cfg(feature = "config-toml")]
    Toml(PathBuf),
    #[cfg(feature = "config-yaml")]
    Yaml(PathBuf),
}

impl Default for ConfigPath {
    fn default() -> Self {
        ConfigPath::Json(PathBuf::from("./goldboot.json"))
    }
}

static VARIANTS: OnceLock<Vec<ConfigPath>> = OnceLock::new();

impl ValueEnum for ConfigPath {
    fn value_variants<'a>() -> &'a [Self] {
        VARIANTS.get_or_init(|| {
            vec![
                #[cfg(feature = "config-json")]
                ConfigPath::Json(PathBuf::from("./goldboot.json")),
                #[cfg(feature = "config-python")]
                ConfigPath::Python(PathBuf::from("./goldboot.py")),
                #[cfg(feature = "config-ron")]
                ConfigPath::Ron(PathBuf::from("./goldboot.ron")),
                #[cfg(feature = "config-toml")]
                ConfigPath::Toml(PathBuf::from("./goldboot.toml")),
                #[cfg(feature = "config-yaml")]
                ConfigPath::Yaml(PathBuf::from("./goldboot.yaml")),
            ]
        })
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match *self {
            #[cfg(feature = "config-json")]
            ConfigPath::Json(_) => Some(PossibleValue::new("json")),
            #[cfg(feature = "config-python")]
            ConfigPath::Python(_) => Some(PossibleValue::new("python")),
            #[cfg(feature = "config-ron")]
            ConfigPath::Ron(_) => Some(PossibleValue::new("ron")),
            #[cfg(feature = "config-toml")]
            ConfigPath::Toml(_) => Some(PossibleValue::new("toml")),
            #[cfg(feature = "config-yaml")]
            ConfigPath::Yaml(_) => Some(PossibleValue::new("yaml")),
        }
    }
}

impl ConfigPath {
    /// Check for a foundry configuration file in the given directory.
    pub fn from_dir(path: impl AsRef<Path>) -> Option<ConfigPath> {
        let path = path.as_ref();

        #[cfg(feature = "config-python")]
        if path.join("goldboot.py").exists() {
            return Some(ConfigPath::Python(path.join("goldboot.py")));
        }

        #[cfg(feature = "config-json")]
        if path.join("goldboot.json").exists() {
            return Some(ConfigPath::Json(path.join("goldboot.json")));
        }

        #[cfg(feature = "config-ron")]
        if path.join("goldboot.ron").exists() {
            return Some(ConfigPath::Ron(path.join("goldboot.ron")));
        }

        #[cfg(feature = "config-toml")]
        if path.join("goldboot.toml").exists() {
            return Some(ConfigPath::Toml(path.join("goldboot.toml")));
        }

        #[cfg(feature = "config-yaml")]
        if path.join("goldboot.yaml").exists() {
            return Some(ConfigPath::Yaml(path.join("goldboot.yaml")));
        } else if path.join("goldboot.yml").exists() {
            return Some(ConfigPath::Yaml(path.join("goldboot.yml")));
        }

        None
    }

    /// Read the configuration file into a new [`Foundry`].
    pub fn load(&self) -> Result<Foundry> {
        Ok(match &self {
            #[cfg(feature = "config-json")]
            Self::Json(path) => serde_json::from_slice(&std::fs::read(path)?)?,
            #[cfg(feature = "config-python")]
            Self::Python(path) => Python::attach(|py| {
                let config_module = PyModule::from_code(
                    py,
                    CString::new(std::fs::read_to_string(path)?)?.as_c_str(),
                    c_str!(""),
                    c_str!("config"),
                )?;

                Ok(todo!())
            })?,
            #[cfg(feature = "config-ron")]
            Self::Ron(path) => ron::de::from_bytes(&std::fs::read(path)?)?,
            #[cfg(feature = "config-toml")]
            Self::Toml(path) => toml::from_str(String::from_utf8(std::fs::read(path)?)?.as_str())?,
            #[cfg(feature = "config-yaml")]
            Self::Yaml(path) => serde_yaml::from_slice(&std::fs::read(path)?)?,
        })
    }

    /// Write a [`Foundry`] to a configuration file.
    pub fn write(&self, foundry: &Foundry) -> Result<()> {
        match &self {
            #[cfg(feature = "config-json")]
            Self::Json(path) => std::fs::write(path, serde_json::to_vec_pretty(foundry)?),
            #[cfg(feature = "config-python")]
            Self::Python(_) => todo!(),
            #[cfg(feature = "config-ron")]
            Self::Ron(path) => std::fs::write(
                path,
                ron::ser::to_string_pretty(foundry, ron::ser::PrettyConfig::new())?.into_bytes(),
            ),
            #[cfg(feature = "config-toml")]
            Self::Toml(path) => std::fs::write(path, toml::to_string_pretty(foundry)?.into_bytes()),
            #[cfg(feature = "config-yaml")]
            Self::Yaml(path) => std::fs::write(path, serde_yaml::to_string(foundry)?.into_bytes()),
        }?;
        Ok(())
    }
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = match self {
            #[cfg(feature = "config-json")]
            ConfigPath::Json(path) => path,
            #[cfg(feature = "config-python")]
            ConfigPath::Python(path) => path,
            #[cfg(feature = "config-ron")]
            ConfigPath::Ron(path) => path,
            #[cfg(feature = "config-toml")]
            ConfigPath::Toml(path) => path,
            #[cfg(feature = "config-yaml")]
            ConfigPath::Yaml(path) => path,
        }
        .to_string_lossy();
        path.fmt(f)
    }
}
