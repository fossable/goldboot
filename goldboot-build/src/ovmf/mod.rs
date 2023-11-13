// UEFI firmwares for various platforms. We include them here to avoid having
// to depend on one provided by the system.

use std::{error::Error, path::Path};

use simple_error::bail;

pub fn write_to(arch: Architecture, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    match &arch {
        Architecture::amd64 => {
            std::fs::write(
                &path,
                zstd::decode_all(std::io::Cursor::new(include_bytes!("x86_64.fd.zst")))?,
            )?;
        }
        Architecture::i386 => {
            std::fs::write(
                &path,
                zstd::decode_all(std::io::Cursor::new(include_bytes!("i386.fd.zst")))?,
            )?;
        }
        Architecture::arm64 => {
            std::fs::write(
                &path,
                zstd::decode_all(std::io::Cursor::new(include_bytes!("aarch64.fd.zst")))?,
            )?;
        }
        _ => bail!("Unsupported architecture"),
    }
    Ok(())
}
