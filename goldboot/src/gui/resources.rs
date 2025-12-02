use image::ImageReader;
use std::io::Cursor;

pub const LOGO_BYTES: &[u8] = include_bytes!("resources/logo-512.png");
pub const ICON_HDD: &[u8] = include_bytes!("resources/icons/hdd.png");
pub const ICON_SSD: &[u8] = include_bytes!("resources/icons/ssd.png");
pub const ICON_NVME: &[u8] = include_bytes!("resources/icons/nvme.png");
pub const ICON_RAM: &[u8] = include_bytes!("resources/icons/ram.png");

pub fn load_image_from_bytes(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess format: {}", e))?
        .decode()
        .map_err(|e| format!("Failed to decode: {}", e))?;

    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let pixels = rgba.as_flat_samples();

    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

pub struct TextureCache {
    pub logo: egui::TextureHandle,
    pub icon_hdd: egui::TextureHandle,
    pub icon_ssd: egui::TextureHandle,
    pub icon_nvme: egui::TextureHandle,
    pub icon_ram: egui::TextureHandle,
}

impl TextureCache {
    pub fn new(ctx: &egui::Context) -> Self {
        Self {
            logo: ctx.load_texture(
                "logo",
                load_image_from_bytes(LOGO_BYTES).expect("Failed to load logo"),
                Default::default(),
            ),
            icon_hdd: ctx.load_texture(
                "icon_hdd",
                load_image_from_bytes(ICON_HDD).expect("Failed to load HDD icon"),
                Default::default(),
            ),
            icon_ssd: ctx.load_texture(
                "icon_ssd",
                load_image_from_bytes(ICON_SSD).expect("Failed to load SSD icon"),
                Default::default(),
            ),
            icon_nvme: ctx.load_texture(
                "icon_nvme",
                load_image_from_bytes(ICON_NVME).expect("Failed to load NVME icon"),
                Default::default(),
            ),
            icon_ram: ctx.load_texture(
                "icon_ram",
                load_image_from_bytes(ICON_RAM).expect("Failed to load RAM icon"),
                Default::default(),
            ),
        }
    }
}
