use crate::{select_device::SelectDeviceView, select_image::SelectImageView};
use gtk4 as gtk;
use gtk4::prelude::*;

pub mod confirm;
pub mod select_device;
pub mod select_image;

fn main() {
	// Configure logging
	env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
}

pub fn start_ui() {
	let app = gtk::Application::builder()
		.application_id("org.example.HelloWorld")
		.build();

	app.connect_activate(|app| {
		let window = gtk::ApplicationWindow::builder()
			.application(app)
			.fullscreened(true)
			.title("Hello, World!")
			.build();

		//let select_image = SelectImageView::new();
		//window.set_child(Some(&select_image.container));

		// Show the window.
		window.show();
	});

	app.run();
}
