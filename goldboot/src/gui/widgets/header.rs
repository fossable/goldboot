use super::super::{resources::TextureCache, theme::Theme};

pub fn render(ui: &mut egui::Ui, textures: &TextureCache, theme: &Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);

        // Display logo (512px wide as per GTK version)
        let logo_size = egui::vec2(512.0, textures.logo.size()[1] as f32 * (512.0 / textures.logo.size()[0] as f32));
        ui.add(egui::Image::new(&textures.logo).max_width(logo_size.x));

        ui.add_space(10.0);

        // Version info (debug builds only)
        #[cfg(debug_assertions)]
        {
            let version = env!("CARGO_PKG_VERSION");
            let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
            let build_date = option_env!("BUILD_DATE").unwrap_or("unknown");

            let version_text = format!("goldboot v{}-{} ({})", version, git_hash, build_date);

            ui.label(
                egui::RichText::new(version_text)
                    .color(theme.text_secondary)
                    .monospace(),
            );

            ui.add_space(10.0);
        }
    });
}
