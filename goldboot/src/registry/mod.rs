pub mod api;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

fn config_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/goldboot/registry.toml")
}

/// Credentials for all configured registries, keyed by registry host.
#[derive(Serialize, Deserialize, Default)]
pub struct RegistryCredentials {
    #[serde(default)]
    pub registries: HashMap<String, RegistryEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RegistryEntry {
    pub token: String,
}

impl RegistryCredentials {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if path.exists() {
            Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, toml::to_string(self)?)?;
        Ok(())
    }

    /// Look up the token for a given registry host.
    pub fn token_for(&self, host: &str) -> Result<&str> {
        self.registries
            .get(host)
            .map(|e| e.token.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Not logged in to registry '{}'. Run: goldboot registry login {}",
                    host,
                    host
                )
            })
    }
}

/// Parse a docker-style image reference: `host/name[:tag]`.
/// Returns `(host, name, tag)`.
pub fn parse_image_ref(reference: &str) -> Result<(String, String, String)> {
    // Split off the tag first
    let (ref_no_tag, tag) = if let Some(pos) = reference.rfind(':') {
        // Make sure the colon isn't part of a host:port before a slash
        let after_colon = &reference[pos + 1..];
        if after_colon.contains('/') {
            // colon was in host:port, no tag
            (reference, "latest")
        } else {
            (&reference[..pos], after_colon)
        }
    } else {
        (reference, "latest")
    };

    // Split host from name at the first slash (host must contain a dot or colon to distinguish)
    let (host, name) = if let Some(slash) = ref_no_tag.find('/') {
        let potential_host = &ref_no_tag[..slash];
        if potential_host.contains('.') || potential_host.contains(':') {
            (potential_host, &ref_no_tag[slash + 1..])
        } else {
            bail!(
                "No registry host found in reference '{}'. Use host/name[:tag]",
                reference
            );
        }
    } else {
        bail!(
            "No registry host found in reference '{}'. Use host/name[:tag]",
            reference
        );
    };

    Ok((host.to_string(), name.to_string(), tag.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ── parse_image_ref ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_ref_host_name_tag() {
        let (host, name, tag) = parse_image_ref("registry.example.com/archlinux:v1").unwrap();
        assert_eq!(host, "registry.example.com");
        assert_eq!(name, "archlinux");
        assert_eq!(tag, "v1");
    }

    #[test]
    fn test_parse_ref_host_name_no_tag() {
        let (host, name, tag) = parse_image_ref("registry.example.com/archlinux").unwrap();
        assert_eq!(host, "registry.example.com");
        assert_eq!(name, "archlinux");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_ref_host_port_name_tag() {
        let (host, name, tag) = parse_image_ref("localhost:5000/myimage:beta").unwrap();
        assert_eq!(host, "localhost:5000");
        assert_eq!(name, "myimage");
        assert_eq!(tag, "beta");
    }

    #[test]
    fn test_parse_ref_host_port_name_no_tag() {
        let (host, name, tag) = parse_image_ref("localhost:5000/myimage").unwrap();
        assert_eq!(host, "localhost:5000");
        assert_eq!(name, "myimage");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_ref_subpath_name() {
        // name can contain slashes (e.g. org/repo style)
        let (host, name, tag) =
            parse_image_ref("registry.example.com/myorg/myimage:latest").unwrap();
        assert_eq!(host, "registry.example.com");
        assert_eq!(name, "myorg/myimage");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_ref_no_host_rejected() {
        assert!(parse_image_ref("archlinux:latest").is_err());
        assert!(parse_image_ref("archlinux").is_err());
    }

    #[test]
    fn test_parse_ref_no_dot_no_colon_host_rejected() {
        // "local" has no dot or colon → not a registry host
        assert!(parse_image_ref("local/archlinux:latest").is_err());
    }

    // ── RegistryCredentials ──────────────────────────────────────────────────

    #[test]
    fn test_credentials_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.toml");

        // Build credentials
        let mut creds = RegistryCredentials::default();
        creds.registries.insert(
            "registry.example.com".to_string(),
            RegistryEntry {
                token: "tok123".to_string(),
            },
        );

        // Serialise
        std::fs::write(&path, toml::to_string(&creds).unwrap()).unwrap();

        // Deserialise
        let loaded: RegistryCredentials =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(loaded.registries["registry.example.com"].token, "tok123");
    }

    #[test]
    fn test_credentials_empty_file_gives_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.toml");
        std::fs::write(&path, "").unwrap();

        let loaded: RegistryCredentials =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(loaded.registries.is_empty());
    }

    #[test]
    fn test_token_for_missing_host_errors() {
        let creds = RegistryCredentials {
            registries: HashMap::new(),
        };
        assert!(creds.token_for("registry.example.com").is_err());
    }

    #[test]
    fn test_token_for_present_host_returns_token() {
        let mut creds = RegistryCredentials::default();
        creds.registries.insert(
            "registry.example.com".to_string(),
            RegistryEntry {
                token: "secret".to_string(),
            },
        );
        assert_eq!(creds.token_for("registry.example.com").unwrap(), "secret");
    }

    #[test]
    fn test_multiple_registries_independent() {
        let mut creds = RegistryCredentials::default();
        creds.registries.insert(
            "a.example.com".to_string(),
            RegistryEntry {
                token: "token_a".to_string(),
            },
        );
        creds.registries.insert(
            "b.example.com".to_string(),
            RegistryEntry {
                token: "token_b".to_string(),
            },
        );

        assert_eq!(creds.token_for("a.example.com").unwrap(), "token_a");
        assert_eq!(creds.token_for("b.example.com").unwrap(), "token_b");
        assert!(creds.token_for("c.example.com").is_err());
    }
}
