//! Commandes Tauri exposées au front.
//!
//! Contrat IPC : chaque commande est reflétée dans `app/src/lib/ipc.ts` et
//! documentée dans `docs/ARCHITECTURE.md` (section IPC).

use palaclips_engine::EngineConfig;

/// Version du moteur.
#[tauri::command]
pub fn engine_version() -> &'static str {
    palaclips_engine::VERSION
}

/// Configuration par défaut du moteur (utilisée par l'écran de réglages au 1er lancement).
#[tauri::command]
pub fn default_config() -> EngineConfig {
    EngineConfig::default()
}
