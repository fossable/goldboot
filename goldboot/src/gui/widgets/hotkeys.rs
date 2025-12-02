use super::super::theme::Theme;

pub fn render(ui: &mut egui::Ui, hotkeys: &[(&str, &str)], theme: &Theme) {
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.add_space(10.0);

        for (key, description) in hotkeys {
            ui.label(
                egui::RichText::new(format!("[{}] {}", key, description))
                    .color(theme.text_secondary)
                    .monospace()
                    .strong(),
            );

            ui.add_space(20.0);
        }
    });

    ui.add_space(10.0);
}
