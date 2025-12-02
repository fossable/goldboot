use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;

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

        // Warning
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Are you sure?")
                    .color(theme.text_secondary)
                    .strong()
                    .size(16.0),
            );
        });

        ui.add_space(20.0);

        // Progress bar (400px wide as per GTK, centered)
        ui.vertical_centered(|ui| {
            let progress_text = format!("{}%", (state.confirm_progress * 100.0) as i32);

            let progress_bar = egui::ProgressBar::new(state.confirm_progress)
                .show_percentage()
                .text(progress_text);

            ui.add_sized([400.0, 20.0], progress_bar);

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Press Enter 100 times to confirm")
                    .color(theme.text_secondary)
                    .size(12.0),
            );
        });

        // Check for Enter key press
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            state.confirm_progress += 0.01;
            if state.confirm_progress >= 1.0 {
                state.confirm_progress = 1.0;

                // Initialize write progress for demo (10GB image)
                // TODO: Use actual image size from selected_image
                state.init_write_progress(10 * 1024 * 1024 * 1024);

                // Navigate to ApplyImage screen
                *screen = Screen::ApplyImage;
            }
        }

        ui.add_space(20.0);

        // Hotkeys footer
        let hotkeys = vec![("Esc", "Quit"), ("Enter", "Confirm (hold)")];
        widgets::hotkeys::render(ui, &hotkeys, theme);
    });
}
