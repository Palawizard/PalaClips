use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    pub schema_version: u32,
    pub capture: CaptureTarget,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub replay: ReplaySettings,
    pub storage: StorageSettings,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            capture: CaptureTarget::default(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            replay: ReplaySettings::default(),
            storage: StorageSettings::default(),
        }
    }
}

impl EngineConfig {
    pub fn validate(&self) -> Result<()> {
        self.video.validate()?;
        self.audio.validate()?;
        self.replay.validate()?;
        self.storage.validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureTarget {
    Monitor {
        device_name: String,
    },
    Window {
        exe_name: String,
        title_hint: Option<String>,
    },
    #[default]
    PrimaryMonitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
}

impl VideoCodec {
    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC",
            Self::Av1 => "AV1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoQuality {
    Quality,
    #[default]
    Balanced,
    Light,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputResolution {
    Native,
    Height { height: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub fps: u32,
    pub resolution: OutputResolution,
    pub codec: VideoCodec,
    pub quality: VideoQuality,
    pub bitrate_kbps: u32,
    pub keyframe_interval_secs: f32,
    pub capture_cursor: bool,
    pub draw_border: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            fps: 60,
            resolution: OutputResolution::Native,
            codec: VideoCodec::H264,
            quality: VideoQuality::Balanced,
            bitrate_kbps: 40_000,
            keyframe_interval_secs: 1.0,
            capture_cursor: true,
            draw_border: false,
        }
    }
}

impl VideoSettings {
    pub fn bitrate_kbps_for(&self, width: u32, height: u32) -> u32 {
        let bits_per_pixel = match self.quality {
            VideoQuality::Quality => 0.30,
            VideoQuality::Balanced => 0.18,
            VideoQuality::Light => 0.08,
            VideoQuality::Custom => return self.bitrate_kbps,
        };
        let kbps = width as f64 * height as f64 * self.fps as f64 * bits_per_pixel / 1_000.0;
        (kbps.round() as u32).clamp(1_000, 200_000)
    }

    pub fn validate(&self) -> Result<()> {
        if !(10..=240).contains(&self.fps) {
            return Err(EngineError::InvalidConfig(
                "les images par seconde doivent être entre 10 et 240".into(),
            ));
        }
        if self.quality == VideoQuality::Custom && !(1_000..=200_000).contains(&self.bitrate_kbps) {
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

pub const SYNC_OFFSET_RANGE_MS: std::ops::RangeInclusive<i32> = -1_000..=1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppTrack {
    pub exe_name: String,
    pub label: String,
    pub enabled: bool,
    #[serde(default)]
    pub sync_offset_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub record_full_mix: bool,
    pub full_mix_label: String,
    pub output_device_id: Option<String>,
    pub full_mix_sync_offset_ms: i32,
    pub record_microphone: bool,
    pub microphone_label: String,
    pub microphone_device_id: Option<String>,
    pub microphone_gain_db: f32,
    pub microphone_sync_offset_ms: i32,
    pub app_tracks: Vec<AppTrack>,
    pub max_app_tracks: usize,
    pub auto_game_track: bool,
    pub game_label: String,
    pub game_sync_offset_ms: i32,
    pub game_exe: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            record_full_mix: true,
            full_mix_label: "Mix complet".into(),
            output_device_id: None,
            full_mix_sync_offset_ms: 0,
            record_microphone: true,
            microphone_label: "Micro".into(),
            microphone_device_id: None,
            microphone_gain_db: 0.0,
            microphone_sync_offset_ms: 0,
            app_tracks: Vec::new(),
            max_app_tracks: 6,
            auto_game_track: true,
            game_label: "Jeu".into(),
            game_sync_offset_ms: 0,
            game_exe: None,
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
        let labels = [
            self.full_mix_label.as_str(),
            self.microphone_label.as_str(),
            self.game_label.as_str(),
        ]
        .into_iter()
        .chain(self.app_tracks.iter().map(|track| track.label.as_str()));
        for label in labels {
            if label.trim().is_empty() {
                return Err(EngineError::InvalidConfig(
                    "le nom d'une piste ne peut pas être vide".into(),
                ));
            }
        }
        let offsets = [
            self.full_mix_sync_offset_ms,
            self.microphone_sync_offset_ms,
            self.game_sync_offset_ms,
        ]
        .into_iter()
        .chain(self.app_tracks.iter().map(|track| track.sync_offset_ms));
        for offset in offsets {
            if !SYNC_OFFSET_RANGE_MS.contains(&offset) {
                return Err(EngineError::InvalidConfig(
                    "le décalage de synchro d'une piste doit être entre -1000 et +1000 ms".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn track_count(&self) -> usize {
        usize::from(self.record_full_mix)
            + usize::from(self.record_microphone)
            + self.app_tracks.iter().filter(|t| t.enabled).count()
            + usize::from(self.auto_game_track)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStorage {
    Ram,
    Disk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplaySettings {
    pub arm_on_startup: bool,
    pub duration_secs: u32,
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    pub clips_dir: Option<std::path::PathBuf>,
    pub game_names: std::collections::BTreeMap<String, String>,
}

impl StorageSettings {
    fn validate(&self) -> Result<()> {
        if let Some(dir) = &self.clips_dir
            && !dir.is_absolute()
        {
            return Err(EngineError::InvalidConfig(
                "le dossier des clips doit être un chemin complet (ex. D:\\Clips)".into(),
            ));
        }
        for name in self.game_names.values() {
            if name.trim().is_empty() {
                return Err(EngineError::InvalidConfig(
                    "un nom de jeu ne peut pas être vide".into(),
                ));
            }
            if crate::games::sanitize(name) != *name {
                return Err(EngineError::InvalidConfig(format!(
                    "« {name} » ne peut pas servir de nom de dossier : retire les caractères < > : \" / \\ | ? * ™ ® ©, les doubles espaces et les points ou espaces en début et fin"
                )));
            }
            if is_reserved_folder_name(name) {
                return Err(EngineError::InvalidConfig(format!(
                    "« {name} » est un nom réservé par Windows, choisis-en un autre"
                )));
            }
        }
        Ok(())
    }
}

fn is_reserved_folder_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes().get(3..), Some([b'1'..=b'9'])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        EngineConfig::default().validate().expect("valid default");
    }

    fn with_game_name(name: &str) -> EngineConfig {
        let mut config = EngineConfig::default();
        config
            .storage
            .game_names
            .insert("game.exe".into(), name.into());
        config
    }

    #[test]
    fn game_names_must_be_valid_folder_names() {
        with_game_name("Rocket League")
            .validate()
            .expect("plain name");
        with_game_name("Tom & Jerry").validate().expect("ampersand");
        assert!(with_game_name("").validate().is_err());
        assert!(with_game_name("   ").validate().is_err());
        assert!(with_game_name("Half-Life: Alyx").validate().is_err());
        assert!(with_game_name("CS2 ").validate().is_err());
        assert!(with_game_name("a/b").validate().is_err());
        assert!(with_game_name("Rocket  League").validate().is_err());
    }

    #[test]
    fn windows_reserved_names_are_refused() {
        for name in ["CON", "nul", "Aux.txt", "COM1", "lpt9"] {
            assert!(with_game_name(name).validate().is_err(), "{name}");
        }
        for name in ["Console", "COM", "COM10", "Nullpomme", "LPT0"] {
            with_game_name(name).validate().expect(name);
        }
    }

    #[test]
    fn clips_dir_must_be_absolute() {
        let mut config = EngineConfig::default();
        config.storage.clips_dir = Some("Clips".into());
        assert!(config.validate().is_err());
        config.storage.clips_dir = Some("D:\\Clips".into());
        config.validate().expect("absolute path");
    }

    #[test]
    fn json_round_trip() {
        let cfg = EngineConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: EngineConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg, back);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let back: EngineConfig = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(back, EngineConfig::default());
    }

    #[test]
    fn duplicate_app_is_rejected() {
        let mut cfg = EngineConfig::default();
        for name in ["Discord.exe", "discord.exe"] {
            cfg.audio.app_tracks.push(AppTrack {
                exe_name: name.into(),
                label: "Discord".into(),
                enabled: true,
                sync_offset_ms: 0,
            });
        }
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn sync_offsets_beyond_one_second_are_rejected() {
        let mut cfg = EngineConfig::default();
        cfg.audio.full_mix_sync_offset_ms = -100;
        assert!(cfg.validate().is_ok());
        cfg.audio.microphone_sync_offset_ms = 1_001;
        assert!(cfg.validate().is_err());
        cfg.audio.microphone_sync_offset_ms = 0;
        cfg.audio.game_sync_offset_ms = -1_001;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn empty_track_names_are_rejected() {
        let mut cfg = EngineConfig::default();
        cfg.audio.microphone_label = "  ".into();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn settings_without_track_names_get_the_defaults() {
        let audio: AudioSettings =
            serde_json::from_str(r#"{"record_full_mix":true,"max_app_tracks":4}"#).expect("parse");
        assert_eq!(audio.full_mix_label, "Mix complet");
        assert_eq!(audio.game_label, "Jeu");
        assert_eq!(audio.max_app_tracks, 4);
    }

    #[test]
    fn quality_presets_scale_bitrate_with_pixels_per_second() {
        let mut video = VideoSettings::default();
        assert_eq!(video.bitrate_kbps_for(2560, 1440), 39_813);
        video.quality = VideoQuality::Light;
        assert_eq!(video.bitrate_kbps_for(1920, 1080), 9_953);
        video.quality = VideoQuality::Quality;
        video.fps = 240;
        assert_eq!(video.bitrate_kbps_for(3840, 2160), 200_000);
        video.quality = VideoQuality::Custom;
        video.bitrate_kbps = 12_000;
        assert_eq!(video.bitrate_kbps_for(3840, 2160), 12_000);
    }

    #[test]
    fn out_of_range_fps_is_rejected() {
        let mut cfg = EngineConfig::default();
        cfg.video.fps = 500;
        assert!(cfg.validate().is_err());
    }
}
