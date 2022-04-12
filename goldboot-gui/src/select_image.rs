use goldboot_core::*;
use gtk4 as gtk;
use gtk4::prelude::*;

#[derive(Debug, Clone)]
struct ImageEntry {
    /// The image name
    pub name: String,

    /// The image size in bytes
    pub size: u64,

    /// The image's URL
    pub url: String,

    /// Build timestamp
    pub timestamp: u64,

    /// The base profiles used for the build
    pub profiles: Vec<String>,
}

impl ImageEntry {
    /// Look for images on the filesystem
    pub fn search() -> Vec<ImageEntry> {
        let mut images = Vec::new();

        for search_path in vec!["."] {
            for read_result in std::fs::read_dir(search_path).unwrap()
            /*.filter(|entry| entry.unwrap().path().extension() == Some(".gb"))*/
            {
                if let Ok(image_path) = read_result {
                    if let Ok(image) = goldboot_image::Qcow2::open(image_path.path()) {
                        if let Ok(metadata) =
                            serde_json::from_slice::<ImageMetadata>(&image.header.metadata.data)
                        {
                            images.push(ImageEntry {
                                name: metadata.config.name.clone(),
                                size: 0,
                                url: image_path.path().into_os_string().into_string().unwrap(),
                                timestamp: 0,
                                profiles: vec![],
                            });
                        }
                    }
                }
            }
        }

        return images;
    }
}

pub struct SelectImageView {
    images: Vec<ImageEntry>,
    list_box: gtk::ListBox,
}

impl SelectImageView {
    pub fn new() -> Self {
        let list_box = gtk::ListBox::new();

        // Rescan images
        let images = ImageEntry::search();

        for image in &images {
            list_box.append(&build_row(&image));
        }

        Self { images, list_box }
    }
}

fn build_row(image: &ImageEntry) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);

    //let platform_icon = Container::new(Svg::from_path(""));

    let image_name = gtk::Label::new(Some(&image.name));
    row.append(&image_name);

    let image_size = gtk::Label::new(Some(&image.size.to_string()));
    row.append(&image_size);

    return row;
}
