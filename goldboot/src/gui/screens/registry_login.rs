use super::super::{state::AppState, theme::Theme};

pub fn render(ctx: &egui::Context, state: &mut AppState, theme: &Theme) {
    if !state.show_registry_dialog {
        return;
    }

    egui::Window::new("Registry Login")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Registry Address:");
            ui.text_edit_singleline(&mut state.registry_address);

            ui.add_space(10.0);

            ui.label("Password:");
            let password_edit = egui::TextEdit::singleline(&mut state.registry_password)
                .password(true);
            ui.add(password_edit);

            ui.add_space(20.0);

            ui.horizontal(|ui| {
                if ui.button("Login").clicked() {
                    // TODO: Implement registry login
                    state.show_registry_dialog = false;
                }

                if ui.button("Cancel").clicked() {
                    state.show_registry_dialog = false;
                }
            });
        });

    // Check for Escape key to close dialog
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.show_registry_dialog = false;
    }
}
