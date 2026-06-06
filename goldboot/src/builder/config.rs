use anyhow::{Result, anyhow, bail};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use crate::builder::os::{OsConfig, os_config_from_ron};

/// Represents a builder configuration file path.
///
/// Two filename conventions are supported in the project directory:
///
/// - `goldboot.ron` — name-less. The image name must be supplied by the
///   caller (`--name` on `goldboot build`).
/// - `<name>.goldboot.ron` — name baked into the filename. The image
///   name is inferred and `--name` becomes optional.
#[derive(Clone, Debug)]
pub struct ConfigPath {
    path: PathBuf,
    /// Image name parsed out of the filename, if it was of the form
    /// `<name>.goldboot.ron`.
    inferred_name: Option<String>,
}

impl Default for ConfigPath {
    fn default() -> Self {
        ConfigPath {
            path: PathBuf::from("./goldboot.ron"),
            inferred_name: None,
        }
    }
}

impl ConfigPath {
    /// Locate a builder configuration file in the given directory.
    ///
    /// Accepts either `<dir>/goldboot.ron` or `<dir>/<name>.goldboot.ron`.
    /// If both forms are present in the same directory, returns an error:
    /// the user must pick one. If only the named form is present, the
    /// name is exposed via [`Self::inferred_name`].
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Option<ConfigPath>> {
        let dir = dir.as_ref();
        let read = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let mut bare: Option<PathBuf> = None;
        let mut named: Vec<(PathBuf, String)> = Vec::new();
        for entry in read.flatten() {
            let path = entry.path();
            let filename_owned = match path.file_name().and_then(|n| n.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            if filename_owned == "goldboot.ron" {
                bare = Some(path);
            } else if let Some(stem) = filename_owned.strip_suffix(".goldboot.ron") {
                if !stem.is_empty() && stem != "." {
                    named.push((path, stem.to_string()));
                }
            }
        }

        if named.len() > 1 {
            let names: Vec<String> = named.iter().map(|(p, _)| p.display().to_string()).collect();
            bail!(
                "multiple `<name>.goldboot.ron` files in {}: {}. Keep only one.",
                dir.display(),
                names.join(", "),
            );
        }
        match (named.pop(), bare) {
            (Some((p, name)), None) => Ok(Some(ConfigPath {
                path: p,
                inferred_name: Some(name),
            })),
            (None, Some(p)) => Ok(Some(ConfigPath {
                path: p,
                inferred_name: None,
            })),
            (Some(_), Some(_)) => bail!(
                "both `goldboot.ron` and `<name>.goldboot.ron` exist in {}; keep only one",
                dir.display()
            ),
            (None, None) => Ok(None),
        }
    }

    /// Build a `ConfigPath` for an explicit file path (used by `init`).
    /// The inferred name is taken from the filename when it ends in
    /// `.goldboot.ron`.
    pub fn with_path(path: PathBuf) -> Self {
        let inferred_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.strip_suffix(".goldboot.ron"))
            .filter(|s| !s.is_empty() && *s != ".")
            .map(|s| s.to_string());
        ConfigPath {
            path,
            inferred_name,
        }
    }

    /// Return the directory containing this config file.
    pub fn context_dir(&self) -> PathBuf {
        self.path
            .parent()
            .expect("config path has no parent")
            .to_path_buf()
    }

    /// The image name parsed from a `<name>.goldboot.ron` filename, if any.
    pub fn inferred_name(&self) -> Option<&str> {
        self.inferred_name.as_deref()
    }

    /// Read and deserialize the Ron configuration file. Returns the list
    /// of OS elements; the file contains no top-level wrapper.
    pub fn load(&self) -> Result<Vec<OsConfig>> {
        let content = std::fs::read_to_string(&self.path)?;
        let trimmed = content.trim();

        if trimmed.starts_with('[') {
            // Top-level list of OsConfigs.
            let values: Vec<ron::Value> = ron::from_str(trimmed)
                .map_err(|e| anyhow!("invalid {}: {e}", self.path.display()))?;
            let element_strs = extract_top_level_list_items(trimmed)?;
            if element_strs.len() != values.len() {
                bail!(
                    "internal: parsed {} elements but extracted {} from text",
                    values.len(),
                    element_strs.len()
                );
            }
            return element_strs
                .iter()
                .map(|s| os_config_from_ron(s))
                .collect::<Result<Vec<_>>>();
        }

        // Single OsConfig.
        Ok(vec![os_config_from_ron(trimmed)?])
    }

    /// Write a new Ron configuration file. `elements` is serialised either
    /// as a bare struct (single element) or a top-level list (multiple).
    pub fn write(&self, elements: &[OsConfig]) -> Result<()> {
        let ron_config = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .enumerate_arrays(false)
            .compact_arrays(false);

        let ron_content = if elements.len() == 1 {
            elements[0].0.serialize_ron(&ron_config)?
        } else {
            let parts: Vec<String> = elements
                .iter()
                .map(|e| e.0.serialize_ron(&ron_config))
                .collect::<Result<Vec<_>>>()?;
            format!("[\n{}\n]", parts.join(",\n"))
        };

        std::fs::write(&self.path, ron_content.as_bytes())?;
        Ok(())
    }
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.to_string_lossy().fmt(f)
    }
}

/// Split the items of a top-level `[ItemA(..), ItemB(..), ...]` list,
/// preserving each item's leading struct name (which `ron::Value` would
/// discard).
fn extract_top_level_list_items(src: &str) -> Result<Vec<String>> {
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'[' {
        bail!("expected `[` at start of list");
    }
    i += 1;

    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '(' | '[' | '{' => {
                if depth == 0 && start.is_none() && !c.is_whitespace() {
                    start = Some(find_token_start(bytes, i));
                }
                depth += 1;
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start.take() {
                        out.push(src[s..=i].to_string());
                    }
                } else if depth < 0 {
                    return Ok(out);
                }
            }
            ',' if depth == 0 => {}
            _ => {
                if depth == 0 && start.is_none() && !c.is_whitespace() {
                    start = Some(find_token_start(bytes, i));
                }
            }
        }
        i += 1;
    }
    bail!("unterminated list")
}

fn find_token_start(bytes: &[u8], i: usize) -> usize {
    let mut j = i;
    while j > 0 {
        let prev = bytes[j - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
            j -= 1;
        } else {
            break;
        }
    }
    j
}
