//! Application Tauri de PalaClips.
//!
//! Rôle : piloter `palaclips-engine` (commandes) et relayer ses événements à l'UI.
//! Aucune logique de capture ici.
//!
//! Cycle de vie visé (phase 5, voir docs/ARCHITECTURE.md § Légèreté) :
//! - l'app vit dans la zone de notification ;
//! - la fenêtre principale est **créée à la demande** et **détruite** à la fermeture
//!   (le WebView2 disparaît, il ne reste qu'un processus natif) ;
//! - `RunEvent::ExitRequested` sans code → `api.prevent_exit()` ; on ne quitte que
//!   via le menu « Quitter » de la zone de notification.
//!
//! État actuel (phase 0) : fenêtre unique, sortie normale à la fermeture.

mod commands;

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// Libellé de la fenêtre principale.
pub const MAIN_WINDOW: &str = "main";

/// Point d'entrée de l'application.
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
            // Pas de panique silencieuse : on journalise puis on quitte proprement.
            eprintln!("PalaClips n'a pas pu démarrer : {err}");
            std::process::exit(1);
        });
}

/// Ouvre (ou met au premier plan) la fenêtre principale.
pub fn open_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::Manager;

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
