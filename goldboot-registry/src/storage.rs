//! On-disk layout for hosted images: `<data_dir>/<name>/<tag>.gb`.
//!
//! Name and tag strings come from URL path parameters, so they are
//! validated against a strict allow-list before being concatenated into a
//! filesystem path. This is the only barrier between an attacker-controlled
//! path component and the host's filesystem.

use anyhow::{Context, Result, bail};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Maximum length of a name or tag (in bytes). Keeps the resulting path
/// reasonable on every common filesystem.
const MAX_COMPONENT_LEN: usize = 64;

/// Allowed character set: ASCII letters, digits, dot, dash, underscore.
fn validate_component(s: &str) -> Result<()> {
    if s.is_empty() {
        bail!("empty component");
    }
    if s.len() > MAX_COMPONENT_LEN {
        bail!("component '{}' exceeds {} bytes", s, MAX_COMPONENT_LEN);
    }
    if s == "." || s == ".." {
        bail!("reserved component: '{}'", s);
    }
    for (i, b) in s.bytes().enumerate() {
        let ok = b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_');
        if !ok {
            bail!("invalid byte 0x{:02x} at position {} in '{}'", b, i, s);
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct Storage {
    data_dir: PathBuf,
}

impl Storage {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("create data_dir {}", data_dir.display()))?;
        Ok(Self { data_dir })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Build a path to a hosted image. Performs strict validation of
    /// `name` and `tag` to prevent path traversal.
    pub fn image_path(&self, name: &str, tag: &str) -> Result<PathBuf> {
        validate_component(name).context("invalid image name")?;
        validate_component(tag).context("invalid image tag")?;
        Ok(self.data_dir.join(name).join(format!("{tag}.gb")))
    }

    /// Open an existing image for reading. Returns the file handle and
    /// its on-disk length.
    pub fn open(&self, name: &str, tag: &str) -> Result<(fs::File, u64)> {
        let path = self.image_path(name, tag)?;
        let file = fs::File::open(&path)
            .with_context(|| format!("open image {}", path.display()))?;
        let len = file.metadata()?.len();
        Ok((file, len))
    }

    /// Atomically write a new image by streaming `body` into a sibling
    /// temp file, then renaming it into place.
    pub fn put(&self, name: &str, tag: &str, mut body: impl Read, _expected_len: Option<u64>) -> Result<u64> {
        let final_path = self.image_path(name, tag)?;
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = final_path.with_extension("gb.partial");
        let mut tmp_file = fs::File::create(&tmp_path)
            .with_context(|| format!("create temp {}", tmp_path.display()))?;
        let mut total = 0u64;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = body.read(&mut buf)?;
            if n == 0 {
                break;
            }
            tmp_file.write_all(&buf[..n])?;
            total += n as u64;
        }
        tmp_file.sync_all()?;
        drop(tmp_file);
        fs::rename(&tmp_path, &final_path)?;
        Ok(total)
    }

    /// List every (name, tag) pair currently in the data directory.
    pub fn list(&self) -> Result<Vec<(String, String)>> {
        let mut out = Vec::new();
        if !self.data_dir.exists() {
            return Ok(out);
        }
        for name_entry in fs::read_dir(&self.data_dir)? {
            let name_entry = name_entry?;
            if !name_entry.file_type()?.is_dir() {
                continue;
            }
            let name_os = name_entry.file_name();
            let Some(name) = name_os.to_str() else {
                continue;
            };
            if validate_component(name).is_err() {
                continue;
            }
            for tag_entry in fs::read_dir(name_entry.path())? {
                let tag_entry = tag_entry?;
                if !tag_entry.file_type()?.is_file() {
                    continue;
                }
                let fname_os = tag_entry.file_name();
                let Some(fname) = fname_os.to_str() else {
                    continue;
                };
                if let Some(tag) = fname.strip_suffix(".gb") {
                    if validate_component(tag).is_ok() {
                        out.push((name.to_string(), tag.to_string()));
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn validates_components() {
        assert!(validate_component("alpine").is_ok());
        assert!(validate_component("alpine-3.20_test").is_ok());
        assert!(validate_component("v1.2.3").is_ok());
        assert!(validate_component("").is_err());
        assert!(validate_component(".").is_err());
        assert!(validate_component("..").is_err());
        assert!(validate_component("a/b").is_err());
        assert!(validate_component("a\0b").is_err());
        assert!(validate_component(&"x".repeat(65)).is_err());
        assert!(validate_component("with space").is_err());
        assert!(validate_component("emoji😀").is_err());
    }

    #[test]
    fn image_path_rejects_traversal() {
        let dir = tempdir().unwrap();
        let s = Storage::new(dir.path()).unwrap();
        assert!(s.image_path("..", "x").is_err());
        assert!(s.image_path("x", "..").is_err());
        assert!(s.image_path("a/b", "x").is_err());
        assert!(s.image_path("x", "a/b").is_err());
    }

    #[test]
    fn put_and_open_round_trip() {
        let dir = tempdir().unwrap();
        let s = Storage::new(dir.path()).unwrap();
        let body = b"hello world".to_vec();
        let len = s.put("img", "tag", body.as_slice(), None).unwrap();
        assert_eq!(len, body.len() as u64);

        let (mut f, n) = s.open("img", "tag").unwrap();
        assert_eq!(n, len);
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, body);

        let listing = s.list().unwrap();
        assert_eq!(listing, vec![("img".into(), "tag".into())]);
    }
}
