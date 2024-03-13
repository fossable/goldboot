use crate::gui::load_png;
use crate::library::ImageLibrary;
use gdk4 as gdk;
use goldboot_image::ImageHandle;
use gtk::glib;
use gtk4 as gtk;
use gtk4::{prelude::*, EventControllerKey};
use tracing::info;
use ubyte::ToByteUnit;

pub fn init(window: &'static gtk::ApplicationWindow) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 5);
    if crate::built_info::PROFILE == "debug" {
        let version = gtk::Label::new(Some(&format!(
            "goldboot v{}-{} ({})",
            crate::built_info::PKG_VERSION,
            if crate::built_info::GIT_DIRTY.unwrap() {
                format!(
                    "{}_dirty",
                    crate::built_info::GIT_COMMIT_HASH_SHORT.unwrap()
                )
            } else {
                format!("{}", crate::built_info::GIT_COMMIT_HASH_SHORT.unwrap())
            },
            built::util::strptime(crate::built_info::BUILT_TIME_UTC),
        )));
        version.add_css_class("versionLabel");
        container.append(&version);
    }
    {
        let logo = load_png(include_bytes!("resources/logo-512.png").to_vec(), 1603, 512);
        container.append(&logo);
    }
    {
        let prompt = gtk::Label::new(Some("Select an available image below"));
        prompt.add_css_class("promptLabel");
        container.append(&prompt);
    }
    {
        let image_box = gtk::ListBox::new();
        image_box.set_vexpand(true);
        container.append(&image_box);

        let mut images = Vec::new();

        for image in ImageLibrary::load().unwrap() {
            images.push(image.id.clone());
            image_box.append(&create_image_row(&image));
        }

        image_box.connect_row_activated(move |_, row| {
            let image_id = images[row.index() as usize].clone();
            info!("Selected image: {}", image_id);
            crate::gui::select_device::init(&window, image_id);
        });
    }
    {
        let hotkeys = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        container.append(&hotkeys);

        let escape = gtk::Label::new(Some("[Esc] Quit"));
        escape.add_css_class("hotkeyLabel");
        hotkeys.append(&escape);

        let registry_login = gtk::Label::new(Some("[F5] Registry Login"));
        registry_login.add_css_class("hotkeyLabel");
        hotkeys.append(&registry_login);

        let enter = gtk::Label::new(Some("[Enter] Select Image"));
        enter.add_css_class("hotkeyLabel");
        hotkeys.append(&enter);
    }

    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        match key {
            gdk::Key::F5 => {
                let content = gtk::Box::new(gtk::Orientation::Vertical, 5);
                {
                    let address = gtk::Entry::builder().placeholder_text("Address").build();
                    content.append(&address);
                }
                {
                    let password = gtk::Entry::builder()
                        .placeholder_text("Password")
                        .visibility(false)
                        .build();
                    content.append(&password);
                }

                let dialog_controller = EventControllerKey::new();

                let dialog = gtk::Dialog::builder().child(&content).modal(true).build();
                dialog.add_controller(dialog_controller);
                dialog.show();
            }
            gdk::Key::Escape => std::process::exit(0),
            _ => {}
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);

    window.set_child(Some(&container));
    window.show();
}

fn create_image_row(image: &ImageHandle) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    row.add_css_class("listRow");

    // TODO dynamic resource
    // if let Some(config) = &image.config {
    //     if let Some(resource) =
    //         Resources::get(&format!("{}.png", config.get_template_bases().unwrap()[0]))
    //     {
    //         let image = load_png(resource.data.to_vec(), 32, 32);
    //         image.add_css_class("listIcon");
    //         row.append(&image);
    //     }
    // } else {
    //     // TODO encrypted
    // }

    // Image name
    let image_name = gtk::Label::new(Some(&image.primary_header.name()));
    image_name.add_css_class("listEntry");
    image_name.set_hexpand(true);
    image_name.set_halign(gtk::Align::Start);
    row.append(&image_name);

    // Image path
    let image_path = gtk::Label::new(Some(&image.path.to_string_lossy()));
    image_path.add_css_class("listEntry");
    row.append(&image_path);

    // Image timestamp
    // TODO

    // Image size
    let image_size = gtk::Label::new(Some(&image.primary_header.size.bytes().to_string()));
    image_size.add_css_class("listEntry");
    row.append(&image_size);

    row
}
