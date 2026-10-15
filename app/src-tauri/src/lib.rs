mod commands;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub const MAIN_WINDOW: &str = "main";

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::engine_version,
            commands::default_config,
        ])
        .setup(|app| {
            open_main_window(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("PalaClips n'a pas pu démarrer : {err}");
            std::process::exit(1);
        });
}

pub fn open_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("PalaClips")
        .inner_size(1180.0, 760.0)
        .min_inner_size(900.0, 600.0)
        .build()?;
    Ok(())
}
