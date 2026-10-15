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
    pub fn validate(&self) -> Result<()> {
        self.video.validate()?;
        self.audio.validate()?;
        self.replay.validate()?;
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
            bitrate_kbps: 40_000,
            keyframe_interval_secs: 1.0,
            capture_cursor: true,
            draw_border: false,
        }
    }
}

impl VideoSettings {
    pub fn validate(&self) -> Result<()> {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppTrack {
    pub exe_name: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub record_full_mix: bool,
    pub output_device_id: Option<String>,
    pub record_microphone: bool,
    pub microphone_device_id: Option<String>,
    pub microphone_gain_db: f32,
    pub app_tracks: Vec<AppTrack>,
    pub max_app_tracks: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        EngineConfig::default().validate().expect("valid default");
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
            });
        }
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn out_of_range_fps_is_rejected() {
        let mut cfg = EngineConfig::default();
        cfg.video.fps = 500;
        assert!(cfg.validate().is_err());
    }
}
