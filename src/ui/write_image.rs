use gtk4::prelude::*;
use gtk4 as gtk;
use crate::ui::select_image::SelectImageView;
use crate::ui::select_device::SelectDeviceView;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/"]
struct Resources;

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

        // Show the window.
        window.show();
    });

    app.run();
}

enum CurrentView {
    SelectImage(SelectImageView),
    SelectDevice(SelectDeviceView),
}

struct WriteImageView {
    current: CurrentView,
}
