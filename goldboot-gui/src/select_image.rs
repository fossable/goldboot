use goldboot_core::image::{ImageMetadata, ImageLibrary};
use gtk4 as gtk;
use gtk4::prelude::*;

pub struct SelectImageView {
	images: Vec<ImageMetadata>,
	list_box: gtk::ListBox,
	pub container: gtk::Box,
}

impl SelectImageView {
	pub fn new() -> Self {
		let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

		let list_box = gtk::ListBox::new();
		container.append(&gtk::Label::new(Some("Logo")));
		container.append(&list_box);

		// Rescan images
		let mut images = Vec::new();

		for image in ImageLibrary::load().unwrap() {
			list_box.append(&build_row(&image));
			images.push(image);
		}

		Self { images, list_box, container }
	}
}

fn build_row(image: &ImageMetadata) -> gtk::Box {
	let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);

	//let platform_icon = Container::new(Svg::from_path(""));

	let image_name = gtk::Label::new(Some(&image.config.name));
	row.append(&image_name);

	let image_size = gtk::Label::new(Some(&image.size.to_string()));
	row.append(&image_size);

	return row;
}
