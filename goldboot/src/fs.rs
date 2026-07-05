use crate::gpt::ExtendedPartition;
use anyhow::{Result, anyhow};
use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom},
    os::unix::{fs::FileTypeExt, io::AsRawFd},
    path::{Path, PathBuf},
    process::Command,
};
use tracing::{info, warn};

/// `BLKGETSIZE64` — returns the size of a block device in bytes via ioctl.
const BLKGETSIZE64: u64 = 0x80081272;

/// Resolve the partition device path for `parent` at `index`. Linux's naming
/// convention appends `p` between the parent name and the partition number
/// when the parent name ends in a digit (`/dev/nvme0n1` → `/dev/nvme0n1p2`,
/// `/dev/mmcblk0` → `/dev/mmcblk0p2`, `/dev/loop0` → `/dev/loop0p2`),
/// otherwise just the number (`/dev/sda` → `/dev/sda2`).
pub fn partition_device_path(parent: &Path, index: u32) -> PathBuf {
    let name = parent
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let suffix = if name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
        format!("p{index}")
    } else {
        index.to_string()
    };
    let mut out = parent.as_os_str().to_owned();
    out.push(&suffix);
    PathBuf::from(out)
}

fn is_block_device(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.file_type().is_block_device())
        .unwrap_or(false)
}

/// Read the first 4 KiB of the partition (relative to its start) and check
/// for filesystem magic bytes. Returns `Ok(true)` for ext2/3/4 (magic
/// `0xEF53` at offset `0x438` from the partition start).
fn is_ext_filesystem(partition_path: &Path) -> Result<bool> {
    let mut f = OpenOptions::new().read(true).open(partition_path)?;
    f.seek(SeekFrom::Start(1024))?;
    let mut sb = [0u8; 1024];
    f.read_exact(&mut sb)?;
    // The ext superblock magic lives at offset 0x38 within the superblock,
    // which itself starts at byte 1024 from the partition start.
    Ok(sb[0x38] == 0x53 && sb[0x39] == 0xEF)
}

/// Query the kernel's view of a block device's size in bytes via `BLKGETSIZE64`.
fn blkgetsize64(path: &Path) -> Result<u64> {
    let f = OpenOptions::new().read(true).open(path)?;
    let mut size: u64 = 0;
    let ret = unsafe { libc::ioctl(f.as_raw_fd(), BLKGETSIZE64, &mut size as *mut u64) };
    if ret != 0 {
        return Err(anyhow!("BLKGETSIZE64 ioctl failed on {}", path.display()));
    }
    Ok(size)
}

/// Refresh the kernel's per-partition size cache after the GPT entry was
/// changed. `partprobe` (BLKRRPART) only works when no partition on the
/// device is open — in our flow that's usually true, but we also run
/// `partx -u` which uses `BLKPG_RESIZE_PARTITION` to update individual
/// partitions even when siblings are held open. Finally, `udevadm settle`
/// blocks until udev finishes processing the resulting change events so the
/// /dev node is up to date by the time `resize2fs` opens it.
fn refresh_partition_table(parent: &Path) {
    let _ = Command::new("partprobe").arg(parent).status();
    match Command::new("partx").arg("-u").arg(parent).status() {
        Ok(s) if !s.success() => warn!(
            target = %parent.display(),
            status = ?s.code(),
            "partx -u returned non-zero status"
        ),
        Err(err) => warn!(
            target = %parent.display(),
            error = ?err,
            "partx not available; kernel partition size may remain stale"
        ),
        _ => {}
    }
    let _ = Command::new("udevadm").arg("settle").status();
}

/// After [`crate::gpt::extend_last_partition`] grew the GPT entry, refresh the
/// kernel's view of the partition table and resize the filesystem inside the
/// trailing partition to fill the new space.
///
/// Silently no-op when `parent` is not a block device (e.g. deploying to a
/// regular file): there's no partition device node to address and no kernel
/// state to refresh.
pub fn resize_partition_fs(parent: &Path, extended: &ExtendedPartition) -> Result<()> {
    if !is_block_device(parent) {
        info!(
            target = %parent.display(),
            "FS extension skipped: output is not a block device"
        );
        return Ok(());
    }

    refresh_partition_table(parent);

    let partition_path = partition_device_path(parent, extended.index);
    if !partition_path.exists() {
        return Err(anyhow!(
            "Partition device {} does not exist after partx",
            partition_path.display()
        ));
    }

    // Sanity check: confirm the kernel actually picked up the new partition
    // size. Without this guard, a stale kernel size silently turns resize2fs
    // into a no-op (the filesystem is "already" the partition size as far as
    // the kernel knows), and the disk reboots with the same small FS.
    let expected_size = (extended.new_last_lba - extended.first_lba + 1) * 512;
    let kernel_size = blkgetsize64(&partition_path)?;
    if kernel_size != expected_size {
        return Err(anyhow!(
            "Kernel partition size for {} is {} bytes but GPT says {} bytes; \
             partition table refresh failed",
            partition_path.display(),
            kernel_size,
            expected_size,
        ));
    }

    if !is_ext_filesystem(&partition_path)? {
        warn!(
            partition = %partition_path.display(),
            "FS extension skipped: not an ext2/3/4 filesystem (only ext is currently supported)"
        );
        return Ok(());
    }

    info!(
        partition = %partition_path.display(),
        size_bytes = expected_size,
        "Resizing ext filesystem to fill partition"
    );

    // resize2fs refuses to operate on a filesystem that hasn't been checked
    // recently. `-f -p` forces a check in preen mode (auto-fix safe errors,
    // no prompts). Exit codes 0/1/2 indicate clean or auto-corrected; 4+
    // means uncorrected errors and we shouldn't proceed to resize.
    let fsck = Command::new("e2fsck")
        .arg("-f")
        .arg("-p")
        .arg(&partition_path)
        .status()?;
    let fsck_code = fsck.code().unwrap_or(-1);
    if !matches!(fsck_code, 0..=2) {
        return Err(anyhow!(
            "e2fsck {} exited with status {}; refusing to resize",
            partition_path.display(),
            fsck_code,
        ));
    }

    let status = Command::new("resize2fs").arg(&partition_path).status()?;
    if !status.success() {
        return Err(anyhow!(
            "resize2fs {} exited with status {:?}",
            partition_path.display(),
            status.code()
        ));
    }
    Ok(())
}
