use glib::clone;
use gtk::glib;
use gtk4 as gtk;
use gtk4::prelude::*;
use log::info;
use std::error::Error;
use ubyte::ToByteUnit;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/select_device/"]
struct Resources;

pub fn init(window: &'static gtk::ApplicationWindow, image_id: String) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

    {
        let logo = crate::load_png(include_bytes!("../res/logo-512.png").to_vec(), 1603, 512);
        container.append(&logo);
    }
    {
        let label = gtk::Label::new(Some("Select a device below to OVERWRITE"));
        label.add_css_class("promptLabel");
        container.append(&label);
    }
    {
        let device_box = gtk::ListBox::new();
        device_box.set_vexpand(true);
        container.append(&device_box);

        let mut devices = Vec::new();

        for device in
            block_utils::get_all_device_info(block_utils::get_block_devices().unwrap()).unwrap()
        {
            devices.push(device.name.clone());
            device_box.append(&create_device_row(&device));
        }

        device_box.connect_row_activated(move |_, row| {
            crate::confirm::init(
                &window,
                image_id.clone(),
                devices[row.index() as usize].clone(),
            );
        });
    }
    {
        let hotkeys = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        container.append(&hotkeys);

        let escape = gtk::Label::new(Some("[Esc] Quit"));
        escape.add_css_class("hotkeyLabel");
        hotkeys.append(&escape);

        let enter = gtk::Label::new(Some("[Enter] Overwrite"));
        enter.add_css_class("hotkeyLabel");
        hotkeys.append(&enter);
    }

    window.set_child(Some(&container));
}

fn create_device_row(device: &block_utils::Device) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    row.add_css_class("listRow");

    // Device icon
    let device_image = match device.media_type {
        block_utils::MediaType::SolidState => {
            crate::load_png(Resources::get("ssd.png").unwrap().data.to_vec(), 32, 32)
        }
        block_utils::MediaType::Rotational => {
            crate::load_png(Resources::get("hdd.png").unwrap().data.to_vec(), 32, 32)
        }
        block_utils::MediaType::NVME => {
            crate::load_png(Resources::get("nvme.png").unwrap().data.to_vec(), 32, 32)
        }
        block_utils::MediaType::Ram => {
            crate::load_png(Resources::get("ram.png").unwrap().data.to_vec(), 32, 32)
        }
        _ => panic!(),
    };
    row.append(&device_image);

    // Device name
    let device_name = gtk::Label::new(Some(
        format!(
            "{} ({})",
            device.name,
            device.serial_number.clone().unwrap()
        )
        .as_str(),
    ));
    device_name.set_hexpand(true);
    device_name.set_halign(gtk::Align::Start);
    row.append(&device_name);

    // Device capacity
    let device_capacity = gtk::Label::new(Some(&device.capacity.bytes().to_string()));
    row.append(&device_capacity);

    row
}
