use super::super::{state::AppState, theme::Theme};
use crate::registry::{RegistryCredentials, RegistryEntry};

pub fn render(ctx: &egui::Context, state: &mut AppState, _theme: &Theme) {
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

            ui.label("Token:");
            let password_edit =
                egui::TextEdit::singleline(&mut state.registry_password).password(true);
            ui.add(password_edit);

            if let Some(err) = &state.registry_login_error {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::RED, err.clone());
            }

            ui.add_space(20.0);

            ui.horizontal(|ui| {
                if ui.button("Login").clicked() {
                    match RegistryCredentials::load() {
                        Ok(mut creds) => {
                            creds.registries.insert(
                                state.registry_address.clone(),
                                RegistryEntry {
                                    token: state.registry_password.clone(),
                                },
                            );
                            match creds.save() {
                                Ok(()) => {
                                    state.registry_login_error = None;
                                    state.registry_password.clear();
                                    state.show_registry_dialog = false;
                                }
                                Err(e) => {
                                    state.registry_login_error =
                                        Some(format!("Failed to save credentials: {e}"));
                                }
                            }
                        }
                        Err(e) => {
                            state.registry_login_error =
                                Some(format!("Failed to load credentials: {e}"));
                        }
                    }
                }

                if ui.button("Cancel").clicked() {
                    state.registry_login_error = None;
                    state.show_registry_dialog = false;
                }
            });
        });

    // Check for Escape key to close dialog
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.registry_login_error = None;
        state.show_registry_dialog = false;
    }
}
