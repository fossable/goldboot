use gdk4 as gdk;
use gtk4 as gtk;
use gtk4::prelude::*;

pub mod apply_image;
pub mod confirm;
pub mod select_device;
pub mod select_image;

fn main() {
    // Configure logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let app = gtk::Application::builder()
        .application_id("org.goldboot.Gui")
        .build();

    app.connect_startup(|_| load_css());
    app.connect_activate(|app| {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .fullscreened(true)
            .title("goldboot")
            .build();

        // Disable the mouse cursor because our simple UI doesn't need it
        window.set_cursor(Some(&gdk::Cursor::from_name("none", None).unwrap()));

        // Show the UI
        select_image::init(Box::leak(Box::new(window)));
    });

    app.run();
}

fn load_css() {
    // Load the CSS file and add it to the provider
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_bytes!("../res/style.css"));

    // Add the provider to the default screen
    gtk::StyleContext::add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

pub fn load_png(data: Vec<u8>, width: i32, height: i32) -> gtk::Image {
    let loader = gdk_pixbuf::PixbufLoader::with_type("png").unwrap();
    loader.write(&data).unwrap();
    loader.close().unwrap();
    let image = gtk::Image::from_pixbuf(loader.pixbuf().as_ref());
    image.set_size_request(width, height);

    image
}
