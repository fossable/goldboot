use goldboot_core::{
	image::{library::ImageLibrary, GoldbootImage},
	BuildConfig,
};
use gtk4 as gtk;
use gtk4::prelude::*;
use ubyte::ToByteUnit;

pub struct SelectImageView {
	images: Vec<ImageRow>,
	list_box: gtk::ListBox,
	pub container: gtk::Box,
}

struct ImageRow {
	pub container: gtk::Box,

	pub name: String,

	pub public: bool,

	pub remote: bool,
}

impl ImageRow {
	pub fn from_image(image: &GoldbootImage) -> Self {
		let container = gtk::Box::new(gtk::Orientation::Horizontal, 5);

		//let platform_icon = Container::new(Svg::from_path(""));

		// Image name
		let image_name = gtk::Label::new(Some(&image.metadata.config.name));
		container.append(&image_name);

		// Image size
		let image_size = gtk::Label::new(Some(&image.size.gibibytes().to_string()));
		container.append(&image_size);

		// Image timestamp
		// TODO

		Self {
			container,
			name: image.metadata.config.name.clone(),
			public: true,
			remote: false,
		}
	}
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
			let row = ImageRow::from_image(&image);
			list_box.append(&row.container);
			images.push(row);
		}

		Self {
			images,
			list_box,
			container,
		}
	}
}
