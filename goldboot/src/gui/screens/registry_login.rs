use super::super::{state::AppState, theme::Theme};
use crate::registry::Client;
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

            if state.registry_address.starts_with("http://")
                && (!state.registry_username.is_empty() || !state.registry_password.is_empty())
            {
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(0xff, 0x4d, 0x4d),
                    "⚠ HTTP (no TLS): Basic Auth credentials will travel in plaintext",
                );
            }

            ui.add_space(10.0);
            ui.label("Username (optional — required if your registry uses HTTP Basic Auth):");
            ui.text_edit_singleline(&mut state.registry_username);

            ui.add_space(10.0);

            ui.label("Password (optional):");
            let password_edit =
                egui::TextEdit::singleline(&mut state.registry_password).password(true);
            ui.add(password_edit);

            if let Some(err) = &state.registry_login_error {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::RED, err.clone());
            }

            ui.add_space(20.0);

            ui.horizontal(|ui| {
                let connect_clicked = ui
                    .add_enabled(
                        !state.registry_login_in_progress,
                        egui::Button::new(if state.registry_login_in_progress {
                            "Connecting…"
                        } else {
                            "Connect"
                        }),
                    )
                    .clicked();

                if connect_clicked && !state.registry_login_in_progress {
                    spawn_connect(state, ctx.clone());
                }

                if ui.button("Cancel").clicked() {
                    state.registry_login_error = None;
                    state.registry_password.clear();
                    state.show_registry_dialog = false;
                }
            });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.registry_login_error = None;
        state.registry_password.clear();
        state.show_registry_dialog = false;
    }
}

/// Result returned from the background connect thread.
enum ConnectEvent {
    Ok(Arc<Mutex<Client>>),
    Err(String),
}

type ConnectRx = std::sync::mpsc::Receiver<ConnectEvent>;

static CONNECT_RX: std::sync::OnceLock<Mutex<Option<ConnectRx>>> = std::sync::OnceLock::new();

fn connect_rx_slot() -> &'static Mutex<Option<ConnectRx>> {
    CONNECT_RX.get_or_init(|| Mutex::new(None))
}

fn spawn_connect(state: &mut AppState, ctx: egui::Context) {
    state.registry_login_in_progress = true;
    state.registry_login_error = None;

    let address = state.registry_address.clone();
    let username = state.registry_username.clone();
    let password = state.registry_password.clone();
    let auth = if username.is_empty() && password.is_empty() {
        None
    } else {
        Some((username, password))
    };

    let (tx, rx) = std::sync::mpsc::channel::<ConnectEvent>();
    *connect_rx_slot().lock().unwrap() = Some(rx);

    thread::spawn(move || {
        let send = |ev: ConnectEvent| {
            let _ = tx.send(ev);
            ctx.request_repaint();
        };

        let client = match Client::new(&address, auth) {
            Ok(c) => c,
            Err(e) => {
                send(ConnectEvent::Err(format!("bad address: {e}")));
                return;
            }
        };
        // Probe the registry with a list call — this is also what validates
        // any HTTP Basic Auth credentials the user typed.
        match client.list_images() {
            Ok(_) => send(ConnectEvent::Ok(Arc::new(Mutex::new(client)))),
            Err(e) => send(ConnectEvent::Err(format!("connect failed: {e}"))),
        }
    });
}

/// Drain any pending connect events. Called once per frame from app.rs so
/// the GUI thread can react without blocking.
pub fn poll_login_events(state: &mut AppState) {
    let slot = connect_rx_slot();
    let guard = slot.lock().unwrap();
    let Some(rx) = guard.as_ref() else { return };
    while let Ok(ev) = rx.try_recv() {
        match ev {
            ConnectEvent::Ok(client) => {
                state.registry_client = Some(client);
                state.registry_login_in_progress = false;
                state.registry_login_error = None;
                state.registry_password.clear();
                state.show_registry_dialog = false;
                kick_image_list(state);
            }
            ConnectEvent::Err(msg) => {
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
