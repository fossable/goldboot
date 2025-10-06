// UEFI firmwares for various platforms. We include them here to avoid having
// to depend on one provided by the system.

use anyhow::Result;
use anyhow::bail;
use goldboot_image::ImageArch;
use std::path::Path;
use std::path::PathBuf;

// TODO use build script to download these from: https://github.com/retrage/edk2-nightly
#[cfg(feature = "include_ovmf")]
pub fn write(arch: ImageArch, path: impl AsRef<Path>) -> Result<()> {
    match &arch {
        ImageArch::Amd64 => {
            std::fs::write(
                &path,
                zstd::decode_all(std::io::Cursor::new(include_bytes!("x86_64.fd.zst")))?,
            )?;
        }
        ImageArch::I386 => {
            std::fs::write(
                &path,
                zstd::decode_all(std::io::Cursor::new(include_bytes!("i386.fd.zst")))?,
            )?;
        }
        ImageArch::Arm64 => {
            std::fs::write(
                &path,
                zstd::decode_all(std::io::Cursor::new(include_bytes!("aarch64.fd.zst")))?,
            )?;
        }
        _ => bail!("Unsupported architecture"),
    }
    Ok(())
}

pub fn find() -> Option<PathBuf> {
    // TODO
    None
}
