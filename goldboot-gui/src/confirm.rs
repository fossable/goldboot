use gtk::{prelude::*, *};
use gtk4 as gtk;

pub fn init(window: &ApplicationWindow, image_id: String, device_id: String) {
	let progress = ProgressBar::new();
	progress.set_text(Some("Progress Bar"));
	progress.set_show_text(true);
	progress.set_hexpand(true);

	let container = Grid::new();
	container.attach(&progress, 0, 0, 1, 1);
	container.set_row_spacing(12);
	container.set_vexpand(true);
	container.set_hexpand(true);

	window.set_child(Some(&container));
}
