pub mod apply_image;
pub mod confirm;
pub mod registry_login;
pub mod select_device;
pub mod select_image;

use super::{resources::TextureCache, state::AppState, theme::Theme};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    SelectImage,
    SelectDevice,
    Confirm,
    ApplyImage,
}

impl Screen {
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut AppState,
        textures: &TextureCache,
        theme: &Theme,
    ) {
        match self {
            Screen::SelectImage => select_image::render(ui, state, textures, theme, self),
            Screen::SelectDevice => select_device::render(ui, state, textures, theme, self),
            Screen::Confirm => confirm::render(ui, state, textures, theme, self),
            Screen::ApplyImage => apply_image::render(ui, state, textures, theme, self),
        }
    }
}
