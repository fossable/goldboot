//! Blocking HTTP client for the goldboot registry.
//!
//! Defaults to HTTPS. The user can opt out by typing `http://` explicitly in
//! front of the registry address; in that case the client logs a
//! `tracing::warn!` so the operator sees credentials are about to travel in
//! plaintext.
//!
//! Authentication is optional HTTP Basic Auth. The server itself does not
//! authenticate; credentials are forwarded to whatever reverse proxy
//! (typically nginx) sits in front of the registry.
//!
//! Custom CA roots (for homelab self-signed certs) are loaded from
//! `~/.config/goldboot/registry-cas.pem` when present. The client never
//! disables certificate verification.

use crate::registry::protocol::{ImageListResponse, MANIFEST_CONTENT_TYPE, RegistryImageEntry};
use anyhow::{Context, Result, bail};
use goldboot_image::{
    DigestTable, Directory, ImageHandle, ManifestBlob, PrimaryHeader, ProtectedHeader,
    parse_manifest,
};
use reqwest::blocking::{Client as HttpClient, ClientBuilder, RequestBuilder, Response};
use rustls::{ClientConfig, RootCertStore};
use std::{fs::File, io::BufReader, io::Write, path::PathBuf, time::Duration};
use tracing::warn;
use url::Url;

const USER_AGENT: &str = concat!("goldboot/", env!("CARGO_PKG_VERSION"));

pub struct Client {
    base: Url,
    http: HttpClient,
    auth: Option<(String, String)>,
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

/// Build a `rustls::ClientConfig` with bundled Mozilla roots plus any custom
/// CA the user has placed at `custom_ca_path()`. This avoids the platform
/// cert-store verifier (which can fail in minimal environments like UKI mode).
fn build_tls_config() -> Result<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let ca = custom_ca_path();
    if ca.exists() {
        let bytes = std::fs::read(&ca).with_context(|| format!("read {}", ca.display()))?;
        let mut reader = BufReader::new(bytes.as_slice());
        for cert in rustls_pemfile::certs(&mut reader) {
            let cert = cert.with_context(|| format!("parse PEM cert from {}", ca.display()))?;
            roots
                .add(cert)
                .with_context(|| "add custom CA cert to root store")?;
        }
    }

    Ok(ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

impl Client {
    pub fn new(address: &str, auth: Option<(String, String)>) -> Result<Self> {
        let base = registry_root(address)?;
        if base.scheme() == "http" && auth.is_some() {
            warn!(
                address = %address,
                "registry contacted over plain HTTP — Basic Auth credentials will be transmitted in plaintext"
            );
        }

        let tls = build_tls_config().context("failed to build TLS config")?;

        let http = ClientBuilder::new()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            .use_preconfigured_tls(tls)
            .build()
            .context("failed to initialize HTTP client")?;

        Ok(Self { base, http, auth })
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// Attach Basic Auth to a request builder if credentials are configured.
    fn auth(&self, rb: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            Some((u, p)) => rb.basic_auth(u, Some(p)),
            None => rb,
        }
    }

    pub fn list_images(&self) -> Result<Vec<RegistryImageEntry>> {
        let url = self.base.join("images")?;
        let resp = self.auth(self.http.get(url)).send()?;
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
        let resp = self.auth(self.http.get(url)).send()?;
        let resp = resp.error_for_status()?;
        if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
            if ct.to_str().unwrap_or("") != MANIFEST_CONTENT_TYPE {
                warn!("unexpected manifest Content-Type: {:?}", ct);
            }
        }
        let bytes = resp.bytes()?;
        let blob = ManifestBlob::read_from(&mut bytes.as_ref())?;
        if blob.headers_encrypted {
            bail!(
                "registry served an encrypted image — pass the password via your local pull flow"
            );
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
            .auth(self.http.get(url))
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
        let url = self.base.join(&format!("images/{name}/tags/{tag}"))?;
        let body = reqwest::blocking::Body::sized(file, len);
        let resp = self
            .auth(self.http.put(url))
            .header(reqwest::header::CONTENT_LENGTH, len)
            .body(body)
            .send()?;
        resp.error_for_status()?;
        Ok(())
    }

    /// Download an image and reconstruct a valid `.gb` file at `dest`.
    pub fn pull_to_file(&self, name: &str, tag: &str, dest: &std::path::Path) -> Result<()> {
        let manifest_url = self
            .base
            .join(&format!("images/{name}/tags/{tag}/manifest"))?;
        let manifest_resp = self
            .auth(self.http.get(manifest_url))
            .send()?
            .error_for_status()?;
        let manifest_bytes = manifest_resp.bytes()?.to_vec();
        let blob = ManifestBlob::read_from(&mut manifest_bytes.as_slice())?;

        let mut out = File::create(dest)?;
        out.write_all(&blob.primary_bytes)?;
        out.write_all(&blob.protected_bytes)?;

        let mut cluster_resp = self.stream_clusters(name, tag, None)?;
        std::io::copy(&mut cluster_resp, &mut out)?;

        out.write_all(&blob.digest_table_bytes)?;
        out.write_all(&blob.directory_bytes)?;
        out.flush()?;
        drop(out);

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
