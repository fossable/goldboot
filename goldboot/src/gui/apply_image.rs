use glib::clone;
use goldboot_image::ImageHandle;
use gtk::glib;
use gtk4 as gtk;
use gtk4::prelude::*;
use log::info;
use std::error::Error;
use ubyte::ToByteUnit;

pub fn init(window: &'static gtk::ApplicationWindow, image_id: String, device_id: String) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

    window.set_child(Some(&container));
}
