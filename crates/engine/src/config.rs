//! Configuration du moteur, choisie par l'utilisateur dans l'app.
//!
//! Sérialisée en JSON dans `%APPDATA%\PalaClips\settings.json` par l'app
//! (versionnée, avec migration). Le moteur ne lit jamais le disque lui-même :
//! il reçoit un [`EngineConfig`] validé.

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};

/// Version du schéma de configuration. À incrémenter à chaque changement
/// incompatible, avec une migration dans l'app.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Configuration complète du moteur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    /// Version du schéma.
    pub schema_version: u32,
    /// Zone capturée.
    pub capture: CaptureTarget,
    /// Réglages vidéo.
    pub video: VideoSettings,
    /// Réglages audio et pistes.
    pub audio: AudioSettings,
    /// Réglages du replay instantané.
    pub replay: ReplaySettings,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            capture: CaptureTarget::default(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            replay: ReplaySettings::default(),
        }
    }
}

impl EngineConfig {
    /// Vérifie que la configuration est utilisable. Les messages sont affichables tels quels.
    pub fn validate(&self) -> Result<()> {
        self.video.validate()?;
        self.audio.validate()?;
        self.replay.validate()?;
        Ok(())
    }
}

/// Ce que l'on capture.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureTarget {
    /// Un écran, identifié par son nom de périphérique (stable entre redémarrages).
    Monitor {
        /// Nom de périphérique Windows (ex. `\\.\DISPLAY1`).
        device_name: String,
    },
    /// Une fenêtre, retrouvée par le nom de l'exe (pas par HWND, qui change).
    Window {
        /// Nom de l'exécutable propriétaire (ex. `FortniteClient-Win64-Shipping.exe`).
        exe_name: String,
        /// Titre au moment du choix, pour départager plusieurs fenêtres du même exe.
        title_hint: Option<String>,
    },
    /// L'écran principal (valeur par défaut au premier lancement).
    #[default]
    PrimaryMonitor,
}

/// Codec vidéo matériel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    /// H.264 : compatibilité maximale (Discord, navigateurs, éditeurs).
    H264,
    /// HEVC : ~2× plus compact, lecture à vérifier dans la galerie (WebView2).
    Hevc,
    /// AV1 : RTX 40+ / RX 7000+ / Arc.
    Av1,
}

/// Résolution de sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputResolution {
    /// Résolution native de la source.
    Native,
    /// Hauteur cible (largeur calculée en gardant le ratio, arrondie au pair).
    Height {
        /// Hauteur en pixels (ex. 1080).
        height: u32,
    },
}

/// Réglages vidéo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// Images par seconde en sortie (CFR).
    pub fps: u32,
    /// Résolution de sortie.
    pub resolution: OutputResolution,
    /// Codec.
    pub codec: VideoCodec,
    /// Débit cible en kbit/s.
    pub bitrate_kbps: u32,
    /// Intervalle entre images clés, en secondes (précision du replay).
    pub keyframe_interval_secs: f32,
    /// Capturer le curseur.
    pub capture_cursor: bool,
    /// Afficher la bordure jaune de Windows autour de la zone capturée.
    pub draw_border: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            fps: 60,
            resolution: OutputResolution::Native,
            codec: VideoCodec::H264,
            bitrate_kbps: 40_000,
            keyframe_interval_secs: 1.0,
            capture_cursor: true,
            draw_border: false,
        }
    }
}

impl VideoSettings {
    fn validate(&self) -> Result<()> {
        if !(10..=240).contains(&self.fps) {
            return Err(EngineError::InvalidConfig(
                "les images par seconde doivent être entre 10 et 240".into(),
            ));
        }
        if !(1_000..=200_000).contains(&self.bitrate_kbps) {
            return Err(EngineError::InvalidConfig(
                "le débit doit être entre 1 et 200 Mb/s".into(),
            ));
        }
        if !(0.25..=4.0).contains(&self.keyframe_interval_secs) {
            return Err(EngineError::InvalidConfig(
                "l'intervalle entre images clés doit être entre 0,25 et 4 s".into(),
            ));
        }
        if let OutputResolution::Height { height } = self.resolution
            && !(360..=4320).contains(&height)
        {
            return Err(EngineError::InvalidConfig(
                "la hauteur de sortie doit être entre 360 et 4320 px".into(),
            ));
        }
        Ok(())
    }
}

/// Une piste audio dédiée à une application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppTrack {
    /// Nom de l'exécutable (persistance par exe, jamais par PID — CLAUDE.md règle 5).
    pub exe_name: String,
    /// Nom affiché et écrit dans les métadonnées de la piste (ex. « Discord »).
    pub label: String,
    /// Piste active.
    pub enabled: bool,
}

/// Réglages audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// Enregistrer la piste « mix complet » (toujours piste 1 si activée).
    pub record_full_mix: bool,
    /// Périphérique de sortie dont on capture le mix (None = périphérique par défaut, suivi à chaud).
    pub output_device_id: Option<String>,
    /// Enregistrer le micro.
    pub record_microphone: bool,
    /// Périphérique micro (None = micro par défaut, suivi à chaud).
    pub microphone_device_id: Option<String>,
    /// Gain du micro en dB.
    pub microphone_gain_db: f32,
    /// Pistes par application, dans l'ordre d'écriture dans le fichier.
    pub app_tracks: Vec<AppTrack>,
    /// Nombre maximal de pistes par application.
    pub max_app_tracks: usize,
    /// L'appli au premier plan alimente automatiquement la piste « Jeu ».
    pub auto_game_track: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            record_full_mix: true,
            output_device_id: None,
            record_microphone: true,
            microphone_device_id: None,
            microphone_gain_db: 0.0,
            app_tracks: Vec::new(),
            max_app_tracks: 6,
            auto_game_track: true,
        }
    }
}

impl AudioSettings {
    fn validate(&self) -> Result<()> {
        if !(1..=12).contains(&self.max_app_tracks) {
            return Err(EngineError::InvalidConfig(
                "le nombre maximal de pistes d'application doit être entre 1 et 12".into(),
            ));
        }
        if self.app_tracks.len() > self.max_app_tracks {
            return Err(EngineError::InvalidConfig(format!(
                "trop de pistes d'application ({} au maximum)",
                self.max_app_tracks
            )));
        }
        let mut seen = std::collections::HashSet::new();
        for track in &self.app_tracks {
            if !seen.insert(track.exe_name.to_ascii_lowercase()) {
                return Err(EngineError::InvalidConfig(format!(
                    "l'application {} est sélectionnée deux fois",
                    track.exe_name
                )));
            }
        }
        if !(-20.0..=20.0).contains(&self.microphone_gain_db) {
            return Err(EngineError::InvalidConfig(
                "le gain du micro doit être entre -20 et +20 dB".into(),
            ));
        }
        Ok(())
    }

    /// Nombre total de pistes audio qui seront écrites.
    pub fn track_count(&self) -> usize {
        usize::from(self.record_full_mix)
            + usize::from(self.record_microphone)
            + self.app_tracks.iter().filter(|t| t.enabled).count()
            + usize::from(self.auto_game_track)
    }
}

/// Où vit le buffer de replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStorage {
    /// Paquets encodés en RAM (par défaut).
    Ram,
    /// Segments sur disque (pour les longues durées).
    Disk,
}

/// Réglages du replay instantané.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplaySettings {
    /// Replay armé au démarrage de l'app.
    pub arm_on_startup: bool,
    /// Durée conservée, en secondes.
    pub duration_secs: u32,
    /// Stockage du buffer.
    pub storage: ReplayStorage,
}

impl Default for ReplaySettings {
    fn default() -> Self {
        Self {
            arm_on_startup: true,
            duration_secs: 60,
            storage: ReplayStorage::Ram,
        }
    }
}

impl ReplaySettings {
    fn validate(&self) -> Result<()> {
        if !(5..=3_600).contains(&self.duration_secs) {
            return Err(EngineError::InvalidConfig(
                "la durée du replay doit être entre 5 s et 60 min".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_par_defaut_valide() {
        EngineConfig::default().validate().expect("défaut valide");
    }

    #[test]
    fn json_aller_retour() {
        let cfg = EngineConfig::default();
        let json = serde_json::to_string(&cfg).expect("sérialisation");
        let back: EngineConfig = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(cfg, back);
    }

    #[test]
    fn champs_manquants_prennent_le_defaut() {
        let back: EngineConfig = serde_json::from_str("{}").expect("désérialisation");
        assert_eq!(back, EngineConfig::default());
    }

    #[test]
    fn doublon_d_application_refuse() {
        let mut cfg = EngineConfig::default();
        for name in ["Discord.exe", "discord.exe"] {
            cfg.audio.app_tracks.push(AppTrack {
                exe_name: name.into(),
                label: "Discord".into(),
                enabled: true,
            });
        }
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn fps_hors_bornes_refuse() {
        let mut cfg = EngineConfig::default();
        cfg.video.fps = 500;
        assert!(cfg.validate().is_err());
    }
}
