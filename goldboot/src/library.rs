use anyhow::{Result, anyhow, bail};
use goldboot_image::{ImageHandle, ImageRef, validate_host_segment, validate_ref_segment};
use rand::RngExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Return the path to the goldboot build cache directory, creating it if needed.
pub fn cache_dir() -> PathBuf {
    let dir =
        PathBuf::from(std::env::var("HOME").expect("HOME not set")).join(".cache/goldboot/images");
    std::fs::create_dir_all(&dir).expect("failed to create cache directory");
    dir
}

/// Return the persistent qcow2 path for a given context directory.
///
/// The path is stable across runs: `~/.cache/goldboot/images/<sha256(canonical_path)>.qcow2`.
pub fn qcow_cache_path(context_dir: &Path) -> Result<PathBuf> {
    let canonical = context_dir.canonicalize()?;
    let hash = hex::encode(
        Sha256::new()
            .chain_update(canonical.to_string_lossy().as_bytes())
            .finalize(),
    );
    Ok(cache_dir().join(format!("{hash}.qcow2")))
}

/// Return the persistent qcow2 path for the element at `index` of a build.
///
/// Element 0 keeps the historical single-element path so existing caches
/// remain valid; later elements get an index suffix.
pub fn element_qcow_cache_path(context_dir: &Path, index: usize) -> Result<PathBuf> {
    let base = qcow_cache_path(context_dir)?;
    Ok(if index == 0 {
        base
    } else {
        base.with_extension(format!("{index}.qcow2"))
    })
}

/// Return the path of the merged multiboot ("alloy") qcow2 for a context
/// directory.
pub fn alloy_qcow_cache_path(context_dir: &Path) -> Result<PathBuf> {
    Ok(qcow_cache_path(context_dir)?.with_extension("alloy.qcow2"))
}

/// Local image library.
///
/// On-disk layout (Linux/macOS):
///
/// ```text
/// <library>/<name>/<tag>.gb           — locally-built image (host = None)
/// <library>/<host>/<name>/<tag>.gb    — image pulled from / pushed to a registry
/// ```
///
/// `find_all` walks both shapes by inspecting whether each top-level
/// directory holds `.gb` files (local) or further subdirectories
/// (host bucket).
pub struct ImageLibrary {
    pub directory: PathBuf,
}

impl ImageLibrary {
    pub fn open() -> Self {
        let directory = if let Ok(dir) = std::env::var("GOLDBOOT_IMAGE_DIR") {
            PathBuf::from(dir)
        } else if cfg!(any(target_os = "linux", target_os = "macos")) {
            PathBuf::from("/var/lib/goldboot/images")
        } else {
            panic!("Unsupported platform");
        };

        debug!(path =%directory.display(), "Opening image library");

        std::fs::create_dir_all(&directory).expect("failed to create image library");
        ImageLibrary { directory }
    }

    /// Return a fresh, unused path under the library directory suitable
    /// for staging a build before promoting it to its final reference path.
    pub fn temporary(&self) -> PathBuf {
        let name: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        self.directory.join(format!(".tmp-{name}"))
    }

    /// On-disk path for an [`ImageRef`]. **Tag must be set** — all on-disk
    /// addressing requires a concrete tag. Host is optional: when `None`,
    /// the image lives directly under the library root.
    pub fn image_path(&self, r: &ImageRef) -> Result<PathBuf> {
        validate_ref_segment(&r.name).map_err(|e| anyhow!("invalid name: {e}"))?;
        let tag = r
            .tag
            .as_deref()
            .ok_or_else(|| anyhow!("a concrete tag is required to address an image"))?;
        validate_ref_segment(tag).map_err(|e| anyhow!("invalid tag: {e}"))?;
        let parent = match r.host_bare() {
            Some(host) => {
                validate_host_segment(host).map_err(|e| anyhow!("invalid host: {e}"))?;
                self.directory.join(host).join(&r.name)
            }
            None => self.directory.join(&r.name),
        };
        Ok(parent.join(format!("{tag}.gb")))
    }

    /// Move an already-built `.gb` file into the library at the canonical
    /// path for the given reference. Returns the destination path.
    pub fn add_built(&self, staged_path: impl AsRef<Path>, r: &ImageRef) -> Result<PathBuf> {
        let dest = self.image_path(r)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        info!(path = %dest.display(), "Moving image into library");
        std::fs::rename(staged_path.as_ref(), &dest)?;
        Ok(dest)
    }

    /// Delete an image. Tag must be concrete.
    pub fn delete(&self, r: &ImageRef) -> Result<PathBuf> {
        let path = self.image_path(r)?;
        std::fs::remove_file(&path)?;
        prune_empty_parents(&self.directory, &path);
        debug!(path = %path.display(), "Deleted image");
        Ok(path)
    }

    /// Resolve an image reference to a handle.
    ///
    /// - `r.tag = Some(_)`: open the exact `<host?>/<name>/<tag>.gb`.
    /// - `r.tag = None`: list `<host?>/<name>/*.gb` and return the one
    ///   with the highest `PrimaryHeader.timestamp`.
    pub fn find_by_ref(&self, r: &ImageRef) -> Result<ImageHandle> {
        validate_ref_segment(&r.name).map_err(|e| anyhow!("invalid name: {e}"))?;

        let name_dir = match r.host_bare() {
            Some(host) => {
                validate_host_segment(host).map_err(|e| anyhow!("invalid host: {e}"))?;
                self.directory.join(host).join(&r.name)
            }
            None => self.directory.join(&r.name),
        };

        if r.tag.is_some() {
            let path = self.image_path(r)?;
            return ImageHandle::open(&path).map_err(|e| anyhow!("no image '{r}' in library: {e}"));
        }

        let read =
            std::fs::read_dir(&name_dir).map_err(|e| anyhow!("no image '{r}' in library: {e}"))?;
        let mut best: Option<ImageHandle> = None;
        for entry in read {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("gb") {
                continue;
            }
            let handle = match ImageHandle::open(&path) {
                Ok(h) => h,
                Err(e) => {
                    debug!(path = %path.display(), error = ?e, "skipping unreadable image");
                    continue;
                }
            };
            best = Some(match best {
                None => handle,
                Some(prev) if handle.primary_header.timestamp > prev.primary_header.timestamp => {
                    handle
                }
                Some(prev) => prev,
            });
        }
        best.ok_or_else(|| anyhow!("no image '{r}' in library"))
    }

    /// Walk the library tree and return one entry per image, paired with
    /// the host (or `None` for locally-built images that have never been
    /// promoted to a registry).
    ///
    /// Disambiguates layout per directory: if a top-level dir contains
    /// `.gb` files it's treated as a name dir (local); if it contains
    /// further subdirectories those are treated as name dirs under a
    /// host bucket. A directory may contain both — both are reported.
    pub fn find_all(&self) -> Result<Vec<(Option<String>, ImageHandle)>> {
        let mut out = Vec::new();
        let Ok(top_iter) = std::fs::read_dir(&self.directory) else {
            return Ok(out);
        };
        for top_entry in top_iter {
            let top_entry = top_entry?;
            if !top_entry.file_type()?.is_dir() {
                continue;
            }
            let top_name = top_entry.file_name();
            let Some(top_str) = top_name.to_str() else {
                continue;
            };
            // Skip our own temp-staging directories (they start with ".tmp-")
            // and other dotfiles.
            if top_str.starts_with('.') {
                continue;
            }

            for inner in std::fs::read_dir(top_entry.path())? {
                let inner = inner?;
                let inner_path = inner.path();
                let ft = inner.file_type()?;
                if ft.is_file() && inner_path.extension().and_then(|s| s.to_str()) == Some("gb") {
                    // <library>/<name>/<tag>.gb — local image.
                    match ImageHandle::open(&inner_path) {
                        Ok(handle) => out.push((None, handle)),
                        Err(e) => {
                            debug!(path = %inner_path.display(), error = ?e, "skipping unreadable image");
                        }
                    }
                } else if ft.is_dir() {
                    // <library>/<host>/<name>/<tag>.gb — top_str is a host.
                    if validate_host_segment(top_str).is_err() {
                        continue;
                    }
                    for tag_entry in std::fs::read_dir(&inner_path)? {
                        let tag_entry = tag_entry?;
                        let tag_path = tag_entry.path();
                        if tag_path.extension().and_then(|s| s.to_str()) != Some("gb") {
                            continue;
                        }
                        match ImageHandle::open(&tag_path) {
                            Ok(handle) => out.push((Some(top_str.to_string()), handle)),
                            Err(e) => {
                                debug!(path = %tag_path.display(), error = ?e, "skipping unreadable image");
                            }
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// Download a goldboot image over HTTP. Used by the UKI flow.
    #[cfg(feature = "cli")]
    pub fn download(&self, url: String) -> Result<ImageHandle> {
        use crate::cli::progress::ProgressBar;
        use std::fs::File;
        let path = self.directory.join("goldboot-uki.gb");

        let mut rs = reqwest::blocking::get(&url)?;
        if rs.status().is_success() {
            let length = rs
                .content_length()
                .ok_or_else(|| anyhow!("Failed to get content length"))?;

            let mut file = File::create(&path)?;

            info!("Saving goldboot image");
            ProgressBar::Download.copy(&mut rs, &mut file, length)?;
            ImageHandle::open(&path)
        } else {
            bail!("Failed to download");
        }
    }
}

/// Recursively remove empty parent directories of `path` up to (but not
/// including) `root`. Best-effort: failures are silently ignored — they
/// mean another image lives in the directory.
fn prune_empty_parents(root: &Path, path: &Path) {
    let mut cur = path.parent();
    while let Some(dir) = cur {
        if dir == root {
            break;
        }
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
        cur = dir.parent();
    }
}
