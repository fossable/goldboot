use anyhow::{Result, bail};
use binrw::{BinRead, BinReaderExt, io::SeekFrom};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Seek, Write},
    path::Path,
    process::{Command, Stdio},
};
use tracing::debug;

mod header;
pub use header::*;

pub mod levels;
use levels::*;

pub mod snapshot;
use snapshot::Snapshot;

/// Represents a (stripped down) qcow3 file on disk.
#[derive(BinRead, Debug)]
#[brw(big)]
pub struct Qcow3 {
    /// The image header
    pub header: QcowHeader,

    /// List of snapshots present within this qcow
    #[br(seek_before = SeekFrom::Start(header.snapshots_offset), count = header.nb_snapshots)]
    pub snapshots: Vec<Snapshot>,

    /// The "active" L1 table
    #[br(seek_before = SeekFrom::Start(header.l1_table_offset), count = header.l1_size)]
    pub l1_table: Vec<L1Entry>,

    /// The file path
    #[br(ignore)]
    pub path: String,
}

impl Qcow3 {
    /// Open a qcow3 file from the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut file = BufReader::new(File::open(&path)?);
        let mut qcow: Qcow3 = file.read_be()?;
        qcow.path = path.as_ref().to_string_lossy().to_string();

        debug!(qcow = ?qcow, "Opened qcow image");
        Ok(qcow)
    }

    /// Allocate a new qcow3 file.
    pub fn create(path: impl AsRef<Path>, size: u64) -> Result<Self> {
        let path = path.as_ref();

        // If we don't pass an image size that's a power of two, qemu-img will
        // silently round up which is bad.
        assert!(size % 2 == 0, "The image size must be a power of 2");

        debug!(path = ?path, "Creating qcow storage");
        let status = Command::new("qemu-img")
            .args([
                "create",
                "-f",
                "qcow2",
                "-o",
                "cluster_size=65536",
                &path.to_string_lossy().to_string(),
                &format!("{size}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();

        if status.code().unwrap() != 0 {
            bail!("Failed to allocate image with qemu-img");
        }

        Qcow3::open(path)
    }

    /// Count the number of allocated clusters.
    pub fn count_clusters(&self) -> Result<u64> {
        let mut count = 0;

        for l1_entry in &self.l1_table {
            if let Some(l2_table) =
                l1_entry.read_l2(&mut File::open(&self.path)?, self.header.cluster_bits)
            {
                for l2_entry in l2_table {
                    if l2_entry.is_allocated() {
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// Find a snapshot by its unique ID string (e.g. "1").
    pub fn snapshot_by_id(&self, id: &str) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.unique_id == id)
    }

    /// Find a snapshot by its human-readable name.
    pub fn snapshot_by_name(&self, name: &str) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.name == name)
    }

    /// Revert the image to the snapshot with the given name.
    ///
    /// This updates the active L1 table pointer and size in the image header to
    /// point at the snapshot's L1 table, making that snapshot the active state.
    pub fn revert(&self, name: &str) -> Result<()> {
        let snapshot = self
            .snapshots
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{name}' not found"))?;

        debug!(path = %self.path, name, "Reverting to snapshot");

        // Update the virtual disk size if the snapshot recorded one
        let new_size = snapshot
            .extra_data
            .virtual_disk_size
            .unwrap_or(self.header.size);

        let mut file = OpenOptions::new().write(true).open(&self.path)?;

        // Write header.size at byte offset 24 (big-endian u64)
        file.seek(SeekFrom::Start(24))?;
        file.write_all(&new_size.to_be_bytes())?;

        // Write header.l1_size at byte offset 36 (big-endian u32)
        file.seek(SeekFrom::Start(36))?;
        file.write_all(&snapshot.l1_entry_count.to_be_bytes())?;

        // Write header.l1_table_offset at byte offset 40 (big-endian u64)
        file.seek(SeekFrom::Start(40))?;
        file.write_all(&snapshot.l1_table_offset.to_be_bytes())?;

        file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open() -> Result<()> {
        let qcow = Qcow3::open("test/empty.qcow2")?;
        assert_eq!(qcow.header.cluster_bits, 16);
        assert_eq!(qcow.header.size, 1048576); // 1 MiB virtual disk
        assert_eq!(qcow.header.nb_snapshots, 0);
        Ok(())
    }

    #[test]
    fn count_clusters_empty() -> Result<()> {
        // An image with no allocated clusters (all blocks are zero)
        let qcow = Qcow3::open("test/empty.qcow2")?;
        assert_eq!(qcow.count_clusters()?, 0);
        Ok(())
    }

    #[test]
    fn count_clusters_uncompressed() -> Result<()> {
        // small.qcow2 has 2 allocated standard clusters
        let qcow = Qcow3::open("test/small.qcow2")?;
        assert_eq!(qcow.count_clusters()?, 2);
        Ok(())
    }

    #[test]
    fn count_clusters_zlib_compressed() -> Result<()> {
        // compressed_zlib.qcow2 has 64 allocated compressed clusters (full 4 MiB)
        let qcow = Qcow3::open("test/compressed_zlib.qcow2")?;
        assert_eq!(qcow.count_clusters()?, 64);
        Ok(())
    }

    #[test]
    fn count_clusters_zstd_compressed() -> Result<()> {
        // compressed_zstd.qcow2 has 1 allocated compressed cluster (64 KiB)
        let qcow = Qcow3::open("test/compressed_zstd.qcow2")?;
        assert_eq!(qcow.count_clusters()?, 1);
        Ok(())
    }
}
