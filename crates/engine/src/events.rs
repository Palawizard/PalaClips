//! Événements émis par le moteur vers l'UI (relayés par Tauri `emit`).
//!
//! Contrat stable : toute modification doit être reportée dans
//! `app/src/lib/ipc.ts` et dans `docs/ARCHITECTURE.md` (section IPC).

use serde::Serialize;

/// État global du moteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    /// Rien ne tourne.
    Idle,
    /// Replay armé, buffer en cours de remplissage.
    ReplayArmed,
    /// Enregistrement manuel en cours (le replay peut tourner en parallèle).
    Recording,
}

/// Niveau de gravité d'une alerte affichée à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Information.
    Info,
    /// Quelque chose ne va pas mais la capture continue.
    Warning,
    /// La capture est arrêtée ou dégradée.
    Error,
}

/// Événement moteur → UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    /// Changement d'état.
    StateChanged {
        /// Nouvel état.
        state: EngineState,
    },
    /// Un clip (replay ou enregistrement) a été écrit sur le disque.
    ClipSaved {
        /// Chemin absolu du fichier.
        path: String,
        /// Durée en secondes.
        duration_secs: f64,
        /// Temps entre l'appui sur le raccourci et le fichier prêt, en ms.
        save_latency_ms: u64,
    },
    /// Niveaux audio en direct, pour les vumètres (≈ 20 Hz, uniquement si l'UI est ouverte).
    AudioLevels {
        /// (identifiant de piste, crête 0..1).
        levels: Vec<(String, f32)>,
    },
    /// Statistiques du buffer de replay.
    ReplayStats {
        /// Secondes actuellement en mémoire.
        buffered_secs: f64,
        /// Octets occupés par le buffer.
        bytes: u64,
    },
    /// Alerte à afficher (piste muette anormale, source perdue, disque presque plein...).
    Alert {
        /// Gravité.
        severity: Severity,
        /// Code stable (pour l'UI et les tests).
        code: String,
        /// Message en français, affichable tel quel.
        message: String,
    },
}
