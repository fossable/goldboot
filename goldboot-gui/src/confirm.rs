use gtk::{prelude::*, *};
use gtk4 as gtk;

pub fn init(window: &'static ApplicationWindow, image_id: String, device_id: String) {
	let container = gtk::Box::new(gtk::Orientation::Vertical, 5);

	{
		let logo = crate::load_png(include_bytes!("../res/logo-512.png").to_vec(), 1603, 512);
		container.append(&logo);
	}
	{
		let warning = gtk::Label::new(Some("Are you sure?"));
		warning.add_css_class("promptLabel");
		container.append(&warning);
	}

	let progress = ProgressBar::new();
	progress.set_show_text(true);
	progress.set_hexpand(true);
	progress.set_width_request(400);
	container.append(&progress);

	let controller = EventControllerKey::new();
	controller.connect_key_pressed( move |controller, key,_,_|
		{
			match key {
				gdk::Key::Return => {
					progress.set_fraction(progress.fraction() + 0.01);
					if progress.fraction() >= 1.0 {
						window.remove_controller(controller);
						crate::apply_image::init(window, image_id.clone(), device_id.clone());
					}
				},
				gdk::Key::Escape => std::process::exit(0),
				_ => {},
			}
			gtk::Inhibit(false)
		});
	window.add_controller(&controller);

	window.set_child(Some(&container));
}
