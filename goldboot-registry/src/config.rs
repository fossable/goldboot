//! Server configuration loaded from a TOML file.
//!
//! Default location is `/etc/goldboot-registry/config.toml`. Override with
//! `--config <path>`.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub users: BTreeMap<String, UserConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_token_ttl")]
    pub token_ttl_secs: u64,
    #[serde(default = "default_max_upload")]
    pub max_upload_size: u64,
    #[serde(default)]
    pub tls_cert: Option<String>,
    #[serde(default)]
    pub tls_key: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            data_dir: default_data_dir(),
            token_ttl_secs: default_token_ttl(),
            max_upload_size: default_max_upload(),
            tls_cert: None,
            tls_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserConfig {
    /// PHC-formatted argon2id hash of the user's password (e.g. produced by
    /// `goldboot-registry user hash`). NEVER store the plaintext password.
    pub password_hash: String,
    #[serde(default = "default_true")]
    pub pull: bool,
    #[serde(default)]
    pub push: bool,
}

fn default_bind() -> String {
    "0.0.0.0:3000".to_string()
}
fn default_data_dir() -> String {
    "/var/lib/goldboot-registry".to_string()
}
fn default_token_ttl() -> u64 {
    86_400
}
fn default_max_upload() -> u64 {
    32 * 1024 * 1024 * 1024
}
fn default_true() -> bool {
    true
}

impl Config {
    /// Load a config from the given file path. Returns a default config if
    /// the path does not exist (so a fresh install can boot and the
    /// operator can populate the file later).
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            tracing::warn!(path = %path.display(), "config not found; using defaults");
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let cfg: Self = toml::from_str(&text)
            .with_context(|| format!("parse config {}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Save the config to the given path with restrictive permissions
    /// (0640) so password hashes are not world-readable.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(path)?.permissions();
            perm.set_mode(0o640);
            fs::set_permissions(path, perm)?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.server.tls_cert.is_some() != self.server.tls_key.is_some() {
            bail!("tls_cert and tls_key must both be set, or both unset");
        }
        for (name, user) in &self.users {
            if name.is_empty() || name.len() > 64 {
                bail!("user name '{}' must be 1..=64 characters", name);
            }
            if !user.password_hash.starts_with("$argon2") {
                bail!(
                    "user '{}' has a non-argon2 password_hash; refusing to load. \
                     Use `goldboot-registry user hash` to generate one.",
                    name
                );
            }
        }
        Ok(())
    }

    pub fn tls_enabled(&self) -> bool {
        self.server.tls_cert.is_some() && self.server.tls_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut cfg = Config::default();
        cfg.users.insert(
            "alice".into(),
            UserConfig {
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$AAAA$BBBB".into(),
                pull: true,
                push: true,
            },
        );
        let text = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert!(back.users.contains_key("alice"));
        assert!(back.users["alice"].push);
    }

    #[test]
    fn rejects_plaintext_password_hash() {
        let toml = r#"
            [server]
            bind = "0.0.0.0:3000"
            data_dir = "/tmp"
            token_ttl_secs = 60
            max_upload_size = 1024

            [users.alice]
            password_hash = "letmein"
        "#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_mismatched_tls() {
        let mut cfg = Config::default();
        cfg.server.tls_cert = Some("a".into());
        assert!(cfg.validate().is_err());
    }
}
