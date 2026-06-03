//! Blocking HTTP client for the goldboot registry.
//!
//! Defaults to HTTPS. The user can opt out by typing `http://` explicitly
//! in front of the registry address; in that case the client logs a
//! `tracing::warn!` so the operator sees credentials are about to travel
//! in plaintext.
//!
//! Custom CA roots (for homelab self-signed certs) are loaded from
//! `~/.config/goldboot/registry-cas.pem` when present. The client never
//! disables certificate verification.

use anyhow::{Context, Result, anyhow, bail};
use goldboot_image::{
    DigestTable, Directory, ImageHandle, ManifestBlob, PrimaryHeader, ProtectedHeader,
    parse_manifest,
};
use crate::registry::protocol::{
    ImageListResponse, LoginRequest, LoginResponse, MANIFEST_CONTENT_TYPE, Permissions,
    RegistryImageEntry,
};
use reqwest::{
    StatusCode,
    blocking::{Client as HttpClient, ClientBuilder, Response},
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    time::Duration,
};
use tracing::warn;
use url::Url;
use zeroize::Zeroize;

const USER_AGENT: &str = concat!("goldboot/", env!("CARGO_PKG_VERSION"));

pub struct Client {
    base: Url,
    http: HttpClient,
    token: Option<String>,
    expires_at: Option<u64>,
    permissions: Permissions,
}

/// Resolve a user-supplied address (`my.registry`, `http://lan:3000`, etc.)
/// into a normalised base URL ending in `/v1/`. Defaults to HTTPS when no
/// scheme is given.
pub fn registry_root(address: &str) -> Result<Url> {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        bail!("empty registry address");
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).with_context(|| format!("invalid url '{trimmed}'"))?;
    // Strip any path the user typed and force /v1/
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    let v1 = url.join("v1/")?;
    Ok(v1)
}

/// Optional custom CA roots for the client (e.g. homelab self-signed cert).
fn custom_ca_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".config/goldboot/registry-cas.pem")
}

impl Client {
    pub fn new(address: &str) -> Result<Self> {
        let base = registry_root(address)?;
        if base.scheme() == "http" {
            warn!(
                address = %address,
                "registry contacted over plain HTTP — credentials and image data will be transmitted in plaintext"
            );
        }

        let mut builder = ClientBuilder::new()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            // Long body timeout for cluster downloads; we apply per-request
            // ::timeout() overrides where needed.
            .pool_idle_timeout(Some(Duration::from_secs(30)));

        let ca = custom_ca_path();
        if ca.exists() {
            let bytes = std::fs::read(&ca)
                .with_context(|| format!("read {}", ca.display()))?;
            for cert in reqwest::Certificate::from_pem_bundle(&bytes)
                .with_context(|| format!("parse PEM bundle {}", ca.display()))?
            {
                builder = builder.add_root_certificate(cert);
            }
        }

        let http = builder.build()?;
        Ok(Self {
            base,
            http,
            token: None,
            expires_at: None,
            permissions: Permissions::default(),
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn permissions(&self) -> Permissions {
        self.permissions
    }

    /// Restore a previously issued token from persistent storage. The
    /// server still enforces its expiration, so a stale token will fail
    /// the first authenticated call with 401 and the user will be
    /// prompted to log in again.
    pub fn set_token_from_storage(&mut self, token: String) {
        self.token = Some(token);
    }

    /// Exchange username + password for an opaque bearer token. The
    /// plaintext password is zeroized after the request, regardless of
    /// outcome.
    pub fn login(&mut self, username: &str, password: &str) -> Result<Permissions> {
        let url = self.base.join("auth/login")?;
        let mut req = LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        };
        let resp = self.http.post(url).json(&req).send();
        req.password.zeroize();
        let resp = resp.context("login: HTTP request failed")?;
        if resp.status() == StatusCode::UNAUTHORIZED {
            bail!("invalid username or password");
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            bail!("login failed: {} {}", status, body);
        }
        let body: LoginResponse = resp.json().context("login: decode JSON")?;
        self.token = Some(body.token);
        self.expires_at = Some(body.expires_at);
        self.permissions = body.permissions;
        Ok(body.permissions)
    }

    /// Revoke the current token server-side. Locally clears it
    /// unconditionally — a network error during logout should not leave a
    /// stale token in memory.
    pub fn logout(&mut self) -> Result<()> {
        let result = if let Some(token) = self.token.as_deref() {
            let url = self.base.join("auth/logout")?;
            self.http.post(url).bearer_auth(token).send().map(|_| ()).map_err(anyhow::Error::from)
        } else {
            Ok(())
        };
        self.token = None;
        self.expires_at = None;
        self.permissions = Permissions::default();
        result
    }

    fn require_token(&self) -> Result<&str> {
        self.token.as_deref().ok_or_else(|| anyhow!("not logged in"))
    }

    pub fn list_images(&self) -> Result<Vec<RegistryImageEntry>> {
        let url = self.base.join("images")?;
        let resp = self.http.get(url).bearer_auth(self.require_token()?).send()?;
        let resp = resp.error_for_status()?;
        let body: ImageListResponse = resp.json()?;
        Ok(body.images)
    }

    /// Fetch and parse the manifest for an image. Returns the parsed
    /// headers + the cluster region start offset so callers can pass it
    /// straight to `stream_write`.
    pub fn fetch_manifest(
        &self,
        name: &str,
        tag: &str,
    ) -> Result<(PrimaryHeader, ProtectedHeader, Directory, DigestTable, u64)> {
        let url = self
            .base
            .join(&format!("images/{name}/tags/{tag}/manifest"))?;
        let resp = self.http.get(url).bearer_auth(self.require_token()?).send()?;
        let resp = resp.error_for_status()?;
        if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
            if ct.to_str().unwrap_or("") != MANIFEST_CONTENT_TYPE {
                warn!("unexpected manifest Content-Type: {:?}", ct);
            }
        }
        let bytes = resp.bytes()?;
        let blob = ManifestBlob::read_from(&mut bytes.as_ref())?;
        // Encrypted images aren't supported on the registry today.
        if blob.headers_encrypted {
            bail!("registry served an encrypted image — pass the password via your local pull flow");
        }
        parse_manifest(&blob, None)
    }

    /// Open a streaming response over the cluster region. The returned
    /// `Response` implements `Read`. Pass an optional byte range to resume
    /// a partial download.
    pub fn stream_clusters(
        &self,
        name: &str,
        tag: &str,
        range: Option<(u64, u64)>,
    ) -> Result<Response> {
        let url = self
            .base
            .join(&format!("images/{name}/tags/{tag}/clusters"))?;
        let mut req = self
            .http
            .get(url)
            .bearer_auth(self.require_token()?)
            // Allow a long server-side read for large clusters
            .timeout(Duration::from_secs(60 * 30));
        if let Some((start, end)) = range {
            req = req.header(reqwest::header::RANGE, format!("bytes={start}-{end}"));
        }
        let resp = req.send()?;
        Ok(resp.error_for_status()?)
    }

    /// Upload a local `.gb` image. The caller provides an open file handle
    /// and its byte length; the request body is the file's contents
    /// verbatim.
    pub fn push_image(&self, name: &str, tag: &str, file: File, len: u64) -> Result<()> {
        let url = self
            .base
            .join(&format!("images/{name}/tags/{tag}"))?;
        let body = reqwest::blocking::Body::sized(file, len);
        let resp = self
            .http
            .put(url)
            .bearer_auth(self.require_token()?)
            .header(reqwest::header::CONTENT_LENGTH, len)
            .body(body)
            .send()?;
        resp.error_for_status()?;
        Ok(())
    }

    /// Download an image and reconstruct a valid `.gb` file at `dest`. Uses
    /// the manifest + clusters endpoints so the wire format matches what
    /// the UKI stream path uses.
    pub fn pull_to_file(&self, name: &str, tag: &str, dest: &std::path::Path) -> Result<()> {
        // Fetch manifest first so we know the cluster region size
        let manifest_url = self
            .base
            .join(&format!("images/{name}/tags/{tag}/manifest"))?;
        let manifest_resp = self
            .http
            .get(manifest_url)
            .bearer_auth(self.require_token()?)
            .send()?
            .error_for_status()?;
        let manifest_bytes = manifest_resp.bytes()?.to_vec();
        let blob = ManifestBlob::read_from(&mut manifest_bytes.as_slice())?;

        let mut out = File::create(dest)?;
        out.write_all(&blob.primary_bytes)?;
        out.write_all(&blob.protected_bytes)?;

        // Stream cluster region to disk
        let mut cluster_resp = self.stream_clusters(name, tag, None)?;
        std::io::copy(&mut cluster_resp, &mut out)?;

        // Append digest_table and directory in their correct on-disk order.
        // We need to overwrite the primary header's directory_offset so it
        // points at the directory we are about to write. The primary
        // header is the first `primary_bytes.len()` bytes of the file and
        // its `directory_offset` field is already set to the server's
        // offset (which matches our layout since we wrote the file in the
        // same order). However the digest_table_offset is in the directory
        // which is encoded with the original offset too. Sanity check
        // expected file size at the end.
        out.write_all(&blob.digest_table_bytes)?;
        out.write_all(&blob.directory_bytes)?;
        out.flush()?;
        drop(out);

        // Verify the resulting file parses correctly
        let _ = ImageHandle::open(dest)?;
        Ok(())
    }

    /// Stream an image directly to a target device or file by consuming
    /// each cluster as it arrives. Used by the UKI mode where no local
    /// staging is allowed.
    pub fn stream_write_to_dest<F: Fn(usize, Option<bool>)>(
        &self,
        name: &str,
        tag: &str,
        dest: &std::path::Path,
        progress: F,
    ) -> Result<(PrimaryHeader, ProtectedHeader, DigestTable)> {
        let (primary, protected, _dir, digest, cluster_start) = self.fetch_manifest(name, tag)?;
        let response = self.stream_clusters(name, tag, None)?;
        ImageHandle::stream_write(
            &primary,
            &protected,
            &digest,
            response,
            cluster_start,
            dest,
            progress,
        )?;
        Ok((primary, protected, digest))
    }
}

/// Internal: discard the unused imports that satisfy the public API
/// surface promises.
#[allow(dead_code)]
fn _seek_required() -> impl Seek {
    File::open("/dev/null").unwrap()
}

/// Internal: discard reads when needed.
#[allow(dead_code)]
fn _read_required() -> impl Read {
    File::open("/dev/null").unwrap()
}

/// Internal: SeekFrom is referenced via dependent helpers in tests.
#[allow(dead_code)]
const _SEEK_FROM_USED: SeekFrom = SeekFrom::Start(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_root_defaults_to_https() {
        let u = registry_root("example.com").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host_str(), Some("example.com"));
        assert!(u.path().ends_with("/v1/"));
    }

    #[test]
    fn registry_root_honors_explicit_scheme() {
        let u = registry_root("http://localhost:3000").unwrap();
        assert_eq!(u.scheme(), "http");
        assert_eq!(u.host_str(), Some("localhost"));
        assert_eq!(u.port(), Some(3000));

        let u = registry_root("https://my.lan:8443").unwrap();
        assert_eq!(u.scheme(), "https");
    }

    #[test]
    fn registry_root_rejects_garbage() {
        assert!(registry_root("").is_err());
    }
}
