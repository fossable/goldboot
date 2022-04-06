use std::error::Error;
use log::info;

#[derive(Debug, Clone)]
struct DiskEntry {

    pub name: String,

    /// The disk size in bytes
    pub size: u64,

    /// Whether the disk is mounted
    pub mounted: bool,
}

impl DiskEntry {

    pub fn search() -> Result<Vec<DiskEntry>, Box<dyn Error>> {
        let mut disks = Vec::new();

        for disk in block_utils::get_all_device_info(block_utils::get_block_devices()?)? {
            disks.push(DiskEntry {
                name: disk.name.clone(),
                size: disk.capacity,
                mounted: false,
            });
            info!("Found disk: {:?}", disk);
        }

        Ok(disks)
    }
}

pub struct SelectDeviceView {
	devices: Vec<DiskEntry>,
	selected: i32,
}