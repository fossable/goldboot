use gtk4 as gtk;
use gtk4::prelude::*;
use log::info;
use std::error::Error;
use ubyte::ToByteUnit;

pub struct SelectDeviceView {
	pub list_box: gtk::ListBox,
	pub devices: Vec<String>,
	pub container: gtk::Box,
}

impl SelectDeviceView {
	pub fn new(image_id: String) -> Self {
		let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

		let list_box = gtk::ListBox::new();
		list_box.connect_row_activated(|l, row| {});
		container.append(&gtk::Label::new(Some("Logo")));
		container.append(&list_box);

		let mut devices = Vec::new();

		for device in
			block_utils::get_all_device_info(block_utils::get_block_devices().unwrap()).unwrap()
		{
			let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);

			// Device name
			let device_name = gtk::Label::new(Some(
				format!("{} ({})", device.name, device.serial_number.unwrap()).as_str(),
			));
			row.append(&device_name);

			// Device capacity
			let device_capacity = gtk::Label::new(Some(&device.capacity.bytes().to_string()));
			row.append(&device_capacity);

			// Media type
			match device.media_type {
				block_utils::MediaType::SolidState => {}
				block_utils::MediaType::Rotational => {}
				block_utils::MediaType::NVME => {}
				block_utils::MediaType::Ram => {}
				_ => {}
			}

			list_box.append(&row);
			devices.push(device.name);
		}

		Self {
			list_box,
			devices,
			container,
		}
	}
}
