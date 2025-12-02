use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;
use ubyte::ToByteUnit;

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    screen: &mut Screen,
) {
    ui.vertical(|ui| {
        // Header with logo
        widgets::header::render(ui, textures, theme);

        // Prompt
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Select an available image below")
                    .color(theme.text_secondary)
                    .strong()
                    .size(16.0),
            );
        });

        ui.add_space(10.0);

        // Image list with horizontal margins (100px as per GTK)
        ui.horizontal(|ui| {
            ui.add_space(100.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.push_id("image_list", |ui| {
                        let available_width = ui.available_width() - 200.0; // Account for margins

                        egui::Frame::none()
                            .stroke(egui::Stroke::new(3.0, theme.border.linear_multiply(0.75)))
                            .fill(theme.list_bg)
                            .inner_margin(15.0)
                            .show(ui, |ui| {
                                ui.set_width(available_width);

                                if state.images.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No images found")
                                            .color(theme.text_secondary),
                                    );
                                } else {
                                    for image in state.images.iter() {
                                        let is_selected =
                                            state.selected_image.as_ref() == Some(&image.id);

                                        let response = ui.horizontal(|ui| {
                                            ui.add_space(5.0);

                                            // Image name
                                            ui.label(
                                                egui::RichText::new(image.primary_header.name())
                                                    .color(theme.text_primary),
                                            );

                                            ui.add_space(20.0);

                                            // Image path
                                            ui.label(
                                                egui::RichText::new(image.path.to_string_lossy())
                                                    .color(theme.text_primary),
                                            );

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.add_space(5.0);

                                                    // Image size
                                                    ui.label(
                                                        egui::RichText::new(
                                                            image.primary_header.size.bytes().to_string(),
                                                        )
                                                        .color(theme.text_primary),
                                                    );
                                                },
                                            );
                                        });

                                        let response = response.response.interact(egui::Sense::click());

                                        if response.clicked() {
                                            state.selected_image = Some(image.id.clone());
                                            // Navigate to SelectDevice screen
                                            *screen = Screen::SelectDevice;
                                        }

                                        if response.hovered() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }

                                        // Check for Enter key to select
                                        if is_selected && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            *screen = Screen::SelectDevice;
                                        }

                                        ui.add_space(5.0);
                                    }
                                }
                            });
                    });
                });

            ui.add_space(100.0);
        });

        ui.add_space(20.0);

        // Hotkeys footer
        let hotkeys = vec![
            ("Esc", "Quit"),
            ("F5", "Registry Login"),
            ("Enter", "Select Image"),
        ];
        widgets::hotkeys::render(ui, &hotkeys, theme);
    });
}
