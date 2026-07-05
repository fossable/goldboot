use anyhow::{Result, anyhow};
use efivar::{
    boot::{
        BootEntry, BootEntryAttributes, EFIHardDrive, EFIHardDriveType, FilePath, FilePathList,
    },
    efi::{Variable, VariableFlags},
};
use uuid::Uuid;

/// Default EFI binary path (the removable-media fallback) for the current
/// architecture. Every properly-built bootable image populates this path on
/// the ESP, so it's a safe target for a freshly-deployed disk without having
/// to mount the ESP to discover what's actually inside.
#[cfg(target_arch = "aarch64")]
pub const DEFAULT_EFI_PATH: &str = "\\EFI\\BOOT\\BOOTAA64.EFI";
#[cfg(not(target_arch = "aarch64"))]
pub const DEFAULT_EFI_PATH: &str = "\\EFI\\BOOT\\BOOTX64.EFI";

/// Everything the firmware needs to address a single GPT partition (the ESP)
/// via an EFI_DEVICE_PATH HARD_DRIVE node.
///
/// `partition_start_lba` and `partition_size_lba` are both in logical blocks
/// (UEFI spec §10.3.5.1), not bytes.
#[derive(Clone, Debug)]
pub struct EspInfo {
    pub partition_number: u32,
    pub partition_start_lba: u64,
    pub partition_size_lba: u64,
    pub partition_guid: Uuid,
}

fn build_file_path_list(esp: &EspInfo, efi_path: &str) -> FilePathList {
    FilePathList {
        file_path: FilePath {
            path: efi_path.to_owned(),
        },
        hard_drive: EFIHardDrive {
            partition_number: esp.partition_number,
            partition_start: esp.partition_start_lba,
            partition_size: esp.partition_size_lba,
            partition_sig: esp.partition_guid,
            format: 0x02,
            sig_type: EFIHardDriveType::Gpt,
        },
    }
}

/// Find an existing Boot#### entry whose description matches `description` and
/// return its ID, or pick a free slot starting at 0x0100.
fn pick_boot_id(manager: &dyn efivar::VarManager, description: &str) -> Result<(u16, bool)> {
    if let Ok(order) = manager.get_boot_order() {
        for id in &order {
            let var = Variable::new(&format!("Boot{:04X}", id));
            if let Ok((data, _)) = manager.read(&var) {
                if let Ok(entry) = BootEntry::parse(data) {
                    if entry.description == description {
                        return Ok((*id, true));
                    }
                }
            }
        }
    }
    let used: std::collections::HashSet<u16> = manager
        .get_boot_order()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let new_id = (0x0100u16..=0xFFFF)
        .find(|id| !used.contains(id))
        .ok_or_else(|| anyhow!("No free boot entry slots"))?;
    Ok((new_id, false))
}

/// Create or overwrite the `Boot####` entry described by `description` to
/// point at `efi_path` on `esp`, then set `BootNext` to that entry so the
/// firmware will boot it on the next power cycle.
///
/// Returns the boot entry ID that was written.
pub fn register_boot_entry(esp: &EspInfo, description: &str, efi_path: &str) -> Result<u16> {
    let file_path_list = build_file_path_list(esp, efi_path);
    let entry = BootEntry {
        attributes: BootEntryAttributes::LOAD_OPTION_ACTIVE,
        description: description.to_owned(),
        file_path_list: Some(file_path_list),
        optional_data: vec![],
    };

    let mut manager = efivar::system();
    let (boot_id, _existing) = pick_boot_id(&*manager, description)?;
    manager.add_boot_entry(boot_id, entry)?;

    let boot_next_bytes = boot_id.to_le_bytes();
    manager.write(
        &Variable::new("BootNext"),
        VariableFlags::NON_VOLATILE
            | VariableFlags::BOOTSERVICE_ACCESS
            | VariableFlags::RUNTIME_ACCESS,
        &boot_next_bytes,
    )?;

    tracing::info!(
        boot_id = format!("Boot{:04X}", boot_id),
        path = efi_path,
        "Registered EFI boot entry and set BootNext"
    );

    Ok(boot_id)
}

/// Describe what [`register_boot_entry`] would do, without writing anything.
pub fn describe_boot_entry(esp: &EspInfo, description: &str, efi_path: &str) -> Result<String> {
    let file_path_list = build_file_path_list(esp, efi_path);
    let manager = efivar::system();
    let (boot_id, existing) = pick_boot_id(&*manager, description)?;
    let action = if existing {
        "overwrite existing"
    } else {
        "create new"
    };
    Ok(format!(
        "  Boot{:04X}  ({action})  {description}  {file_path_list}\n  BootNext = Boot{:04X}",
        boot_id, boot_id,
    ))
}
