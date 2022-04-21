use crate::levels::{L1Entry, L2Entry};
use crate::*;

use std::convert::TryInto;
use std::io::{self, Read, Seek};

/// A reader for reading from the guest virtual drive. Should be constructed using
/// [`GoldbootImage::reader`].
pub struct Reader<'qcow, 'reader, R>
where
    R: Read + Seek,
{
    qcow: &'qcow GoldbootImage,

    /// inner reader used for reading/seeking in the host file (the qcow itself)
    reader: &'reader mut R,

    /// current position of the reader within the guest
    pos: u64,

    // l1 key and cache. if l1 is being accessed by something with an outdated key,
    // the l1_cache needs to be refreshed before returning.
    l1_key: u64,
    l1_cache: &'qcow L1Entry,
    l2_table_cache: Vec<L2Entry>,

    // l2 key and cache. if l2 is being accessed by something with an outdated key,
    // the l2_cache needs to be refreshed before returning.
    l2_key: u64,
    l2_cache: L2Entry,

    /// the current cluster from which the reader is reading, the size of which __must__ be
    /// equivelant to cluster size
    current_cluster: Box<[u8]>,
}

impl GoldbootImage {
    /// Create a reader for reading from the guest virtual drive
    pub fn reader<'qcow, 'reader, R>(
        &'qcow self,
        reader: &'reader mut R,
    ) -> Reader<'qcow, 'reader, R>
    where
        R: Read + Seek,
    {
        let pos = 0;
        let l1_key = 0;
        let l2_key = 0;
        let qcow = self;
        let l1_cache = self
            .l1_table
            .get(l1_key as usize)
            .expect("No L1 table entries found");
        let l2_table_cache = l1_cache
            .read_l2(reader, qcow.header.cluster_bits)
            .expect("No L2 table found");
        let l2_cache = l2_table_cache
            .get(l2_key as usize)
            .expect("No L2 table entries found")
            .clone();

        let mut current_cluster = vec![0; self.header.cluster_size() as usize].into_boxed_slice();
        l2_cache
            .read_contents(
                reader,
                &mut current_cluster[..],
                qcow.header.compression_type,
            )
            .or_else(|err| Err(err))
            .expect("Failed to read first qcow cluster");

        Reader {
            qcow,
            reader,
            pos,
            l1_cache,
            l2_table_cache,
            l2_cache,
            l1_key,
            l2_key,
            current_cluster,
        }
    }
}

impl<'qcow, 'reader, R> Reader<'qcow, 'reader, R>
where
    R: Read + Seek,
{
    /// Returns the current read position within the guest virtual hard disk
    pub fn guest_pos(&self) -> u64 {
        self.pos
    }

    fn update_l1_cache(&mut self) -> io::Result<()> {
        let l2_entries = self.qcow.header.cluster_size() / (std::mem::size_of::<u64>() as u64);
        let l1_key = (self.pos / self.qcow.header.cluster_size()) / l2_entries;

        if self.l1_key != l1_key {
            self.l1_key = l1_key;
            self.l1_cache = self.qcow.l1_table.get(l1_key as usize).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Read position past end of virtual disk",
                )
            })?;

            self.l2_table_cache = self
                .l1_cache
                .read_l2(self.reader, self.qcow.header.cluster_bits)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "L2 table could not be read")
                })?;
        }

        Ok(())
    }

    fn update_l2_cache(&mut self) -> io::Result<()> {
        let l2_entries = self.qcow.header.cluster_size() / (std::mem::size_of::<u64>() as u64);
        let l2_key = self.pos / self.qcow.header.cluster_size();
        let l2_index = l2_key % l2_entries;

        if self.l2_key != l2_key {
            self.l2_key = l2_key;
            self.update_l1_cache()?;
            if self.l1_cache.l2_offset() != 0 {
                self.l2_cache = self.l2_table_cache[l2_index as usize].clone();
                self.l2_key = l2_key;
            }
        }

        if self.l1_cache.l2_offset() == 0 {
            // empty cluster?
            self.current_cluster.fill(0);
        } else {
            self.l2_cache.read_contents(
                self.reader,
                &mut self.current_cluster[..],
                self.qcow.header.compression_type,
            )?;
        }

        Ok(())
    }
}

impl<'qcow, 'reader, R> Read for Reader<'qcow, 'reader, R>
where
    R: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.update_l2_cache()?;

        let cluster_size = self.qcow.header.cluster_size();
        let pos_in_cluster = self.pos % cluster_size;
        let bytes_remaining_in_cluster = cluster_size - pos_in_cluster;

        let read_len = u64::min(bytes_remaining_in_cluster, buf.len() as u64);
        let read_end: usize = (pos_in_cluster + read_len).try_into().unwrap();
        let pos_in_cluster: usize = pos_in_cluster.try_into().unwrap();

        buf[..read_len as usize]
            .copy_from_slice(&self.current_cluster[pos_in_cluster..read_end]);

        self.pos += read_len;
        let _ = self.update_l2_cache();

        Ok(read_len as usize)
    }
}

impl<'qcow, 'reader, R> Seek for Reader<'qcow, 'reader, R>
where
    R: Read + Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(new_pos) => self.pos = new_pos,
            SeekFrom::Current(rel_offset) => {
                self.pos = self
                    .pos
                    .try_into()
                    .map(|pos: i64| pos + rel_offset)
                    .unwrap_or_else(|_| {
                        ((self.pos as i128) + (rel_offset as i128))
                            .try_into()
                            .unwrap()
                    })
                    .try_into()
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "seek out of range of 64-bit position",
                        )
                    })?;
            }
            SeekFrom::End(from_end) => {
                self.pos = (from_end + (self.qcow.header.size as i64))
                    .try_into()
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "seek out of range of 64-bit position",
                        )
                    })?;
            }
        }

        let _ = self.update_l2_cache();

        Ok(self.pos)
    }
}
