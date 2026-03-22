use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;

pub fn render(
    ui: &mut egui::Ui,
    _state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    _screen: &mut Screen,
) {
    egui::TopBottomPanel::bottom("sudo_confirm_hotkeys")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            let hotkeys = vec![("Esc", "Quit"), ("Y", "Re-invoke")];
            widgets::hotkeys::render(ui, &hotkeys, theme);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                widgets::header::render(ui, textures, theme);

                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);

                    ui.label(
                        egui::RichText::new("Elevated permissions are required to write images.")
                            .color(theme.text_primary)
                            .strong()
                            .size(18.0),
                    );

                    ui.add_space(12.0);

                    ui.label(
                        egui::RichText::new("Re-invoke with sudo?")
                            .color(theme.text_secondary)
                            .size(14.0),
                    );
                });

                if ui.input(|i| {
                    i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::Text(t) if t.eq_ignore_ascii_case("y")))
                }) {
                    let args: Vec<String> = std::env::args().collect();
                    let _ = std::process::Command::new("sudo").args(&args).status();
                    std::process::exit(0);
                }
            });
        });
}
