use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use super::Qcow3;

/// A streaming reader over the virtual disk contents of a [`Qcow3`] image.
///
/// Unallocated clusters are read as all-zeros, matching qemu-img behaviour.
pub struct Qcow3Reader<'a> {
    pub(super) qcow: &'a Qcow3,
    pub(super) file: File,
    pub(super) pos: u64,
}

impl Read for Qcow3Reader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.qcow.header.size {
            return Ok(0);
        }

        let cluster_size = self.qcow.header.cluster_size();
        let l2_entries = cluster_size / 8;

        let cluster_idx = self.pos / cluster_size;
        let offset_in_cluster = (self.pos % cluster_size) as usize;

        let l1_idx = (cluster_idx / l2_entries) as usize;
        let l2_idx = (cluster_idx % l2_entries) as usize;

        // How many bytes remain in the current cluster (capped to virtual disk size and buf)
        let available = (cluster_size as usize - offset_in_cluster)
            .min((self.qcow.header.size - self.pos) as usize)
            .min(buf.len());

        let contents = self.qcow.l1_table
            .get(l1_idx)
            .and_then(|l1| l1.read_l2(&mut self.file, self.qcow.header.cluster_bits))
            .and_then(|l2| l2.into_iter().nth(l2_idx))
            .and_then(|l2e| {
                l2e.read_contents(
                    &mut self.file,
                    cluster_size,
                    self.qcow.header.compression_type,
                )
                .ok()
                .flatten()
            });

        match contents {
            Some(data) => buf[..available].copy_from_slice(&data[offset_in_cluster..][..available]),
            None => buf[..available].fill(0),
        }

        self.pos += available as u64;
        Ok(available)
    }
}

impl Seek for Qcow3Reader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let size = self.qcow.header.size as i64;
        let new_pos = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => size + n,
            SeekFrom::Current(n) => self.pos as i64 + n,
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.pos = new_pos as u64;
        Ok(self.pos)
    }
}
