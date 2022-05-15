use goldboot_core::image::library::ImageLibrary;
use goldboot_core::image::GoldbootImage;
use log::info;
use std::error::Error;
use gtk::glib;
use glib::clone;
use gtk4 as gtk;
use gtk4::prelude::*;
use ubyte::ToByteUnit;

pub fn init(window: &gtk::ApplicationWindow) {
	let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

	window.set_child(Some(&container));
}

