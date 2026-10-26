use tauri_plugin_notification::NotificationExt;

pub use palaclips_engine::chime::{Sound, play};

pub fn toast(app: &tauri::AppHandle, title: &str, body: &str) {
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(%error, "notification non affichée");
    }
}
