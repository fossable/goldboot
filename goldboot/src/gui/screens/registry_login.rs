use super::super::{state::AppState, theme::Theme};
use crate::registry::Client;
#[cfg(not(feature = "uki"))]
use crate::registry::{RegistryCredentials, RegistryEntry};
use std::sync::{Arc, Mutex};
use std::thread;

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

            // HTTPS warning if the user typed http:// explicitly
            if state.registry_address.starts_with("http://") {
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(0xff, 0x4d, 0x4d),
                    "⚠ HTTP (no TLS): credentials and image data will travel in plaintext",
                );
            }

            ui.add_space(10.0);
            ui.label("Username:");
            ui.text_edit_singleline(&mut state.registry_username);

            ui.add_space(10.0);

            ui.label("Password:");
            let password_edit =
                egui::TextEdit::singleline(&mut state.registry_password).password(true);
            ui.add(password_edit);

            if let Some(err) = &state.registry_login_error {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::RED, err.clone());
            }

            ui.add_space(20.0);

            ui.horizontal(|ui| {
                let login_clicked = ui
                    .add_enabled(
                        !state.registry_login_in_progress,
                        egui::Button::new(if state.registry_login_in_progress {
                            "Logging in…"
                        } else {
                            "Login"
                        }),
                    )
                    .clicked();

                if login_clicked && !state.registry_login_in_progress {
                    spawn_login(state, ctx.clone());
                }

                if ui.button("Cancel").clicked() {
                    state.registry_login_error = None;
                    state.registry_password.clear();
                    state.show_registry_dialog = false;
                }
            });
        });

    // Check for Escape key to close dialog
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.registry_login_error = None;
        state.registry_password.clear();
        state.show_registry_dialog = false;
    }
}

/// Login result returned from the background thread.
enum LoginEvent {
    Ok {
        client: Arc<Mutex<Client>>,
        token: String,
    },
    Err(String),
}

/// Channel between background login threads and the UI. Each entry is
/// matched to its dialog state via `Once`. We poll it on every frame from
/// `app.rs`.
type LoginRx = std::sync::mpsc::Receiver<LoginEvent>;

/// Storage for the in-flight login receiver. Held on the AppState would be
/// cleanest, but the dialog module owns its own static for simplicity.
static LOGIN_RX: std::sync::OnceLock<Mutex<Option<LoginRx>>> = std::sync::OnceLock::new();

fn login_rx_slot() -> &'static Mutex<Option<LoginRx>> {
    LOGIN_RX.get_or_init(|| Mutex::new(None))
}

fn spawn_login(state: &mut AppState, ctx: egui::Context) {
    state.registry_login_in_progress = true;
    state.registry_login_error = None;

    let address = state.registry_address.clone();
    let username = state.registry_username.clone();
    let password = state.registry_password.clone();

    let (tx, rx) = std::sync::mpsc::channel::<LoginEvent>();
    *login_rx_slot().lock().unwrap() = Some(rx);

    thread::spawn(move || {
        let send = |ev: LoginEvent| {
            let _ = tx.send(ev);
            ctx.request_repaint();
        };

        let mut client = match Client::new(&address) {
            Ok(c) => c,
            Err(e) => {
                send(LoginEvent::Err(format!("bad address: {e}")));
                return;
            }
        };
        match client.login(&username, &password) {
            Ok(_perms) => {
                let token = client.token().map(|s| s.to_string()).unwrap_or_default();
                send(LoginEvent::Ok {
                    client: Arc::new(Mutex::new(client)),
                    token,
                });
            }
            Err(e) => send(LoginEvent::Err(format!("login failed: {e}"))),
        }
    });
}

/// Drain any pending login events. Called once per frame from app.rs so
/// the GUI thread can react without blocking.
pub fn poll_login_events(state: &mut AppState) {
    let slot = login_rx_slot();
    let guard = slot.lock().unwrap();
    let Some(rx) = guard.as_ref() else { return };
    while let Ok(ev) = rx.try_recv() {
        match ev {
            LoginEvent::Ok { client, token } => {
                state.registry_client = Some(client);
                state.registry_login_in_progress = false;
                state.registry_login_error = None;
                state.registry_password.clear();
                state.show_registry_dialog = false;

                // Persist the token in non-UKI builds. In UKI mode we
                // never write the token to disk — the filesystem is
                // ephemeral and a leaked token in /run is still a leak.
                #[cfg(not(feature = "uki"))]
                {
                    let host = state.registry_address.clone();
                    if let Ok(mut creds) = RegistryCredentials::load() {
                        creds
                            .registries
                            .insert(host, RegistryEntry { token: token.clone() });
                        let _ = creds.save();
                    } else {
                        let mut creds = RegistryCredentials::default();
                        creds
                            .registries
                            .insert(state.registry_address.clone(), RegistryEntry { token: token.clone() });
                        let _ = creds.save();
                    }
                }
                let _ = token;

                kick_image_list(state);
            }
            LoginEvent::Err(msg) => {
                state.registry_login_in_progress = false;
                state.registry_login_error = Some(msg);
            }
        }
    }
}

/// Channel for the image-list refresh.
enum ListEvent {
    Ok(Vec<crate::registry::protocol::RegistryImageEntry>),
    Err(String),
}
static LIST_RX: std::sync::OnceLock<Mutex<Option<std::sync::mpsc::Receiver<ListEvent>>>> =
    std::sync::OnceLock::new();

fn list_rx_slot() -> &'static Mutex<Option<std::sync::mpsc::Receiver<ListEvent>>> {
    LIST_RX.get_or_init(|| Mutex::new(None))
}

fn kick_image_list(state: &mut AppState) {
    let Some(client) = state.registry_client.clone() else {
        return;
    };
    state.registry_list_loading = true;
    state.registry_list_error = None;
    let (tx, rx) = std::sync::mpsc::channel::<ListEvent>();
    *list_rx_slot().lock().unwrap() = Some(rx);
    thread::spawn(move || {
        let result = {
            let client = match client.lock() {
                Ok(c) => c,
                Err(_) => {
                    let _ = tx.send(ListEvent::Err("client poisoned".to_string()));
                    return;
                }
            };
            client.list_images()
        };
        let ev = match result {
            Ok(images) => ListEvent::Ok(images),
            Err(e) => ListEvent::Err(e.to_string()),
        };
        let _ = tx.send(ev);
    });
}

pub fn poll_list_events(state: &mut AppState) {
    let slot = list_rx_slot();
    let guard = slot.lock().unwrap();
    let Some(rx) = guard.as_ref() else { return };
    while let Ok(ev) = rx.try_recv() {
        state.registry_list_loading = false;
        match ev {
            ListEvent::Ok(images) => {
                state.registry_images = images;
            }
            ListEvent::Err(e) => state.registry_list_error = Some(e),
        }
    }
}
