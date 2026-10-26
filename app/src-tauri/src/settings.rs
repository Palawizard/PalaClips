use std::path::{Path, PathBuf};

use palaclips_engine::EngineConfig;
use palaclips_engine::config::{AudioSettings, DefaultTrackNames};
use palaclips_engine::events::Localized;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::{self, Language, LanguageSetting};

pub const SETTINGS_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub engine: EngineConfig,
    pub feedback: Feedback,
    pub hotkeys: crate::hotkeys::Hotkeys,
    pub theme: Theme,
    pub language: LanguageSetting,
    pub onboarded: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_VERSION,
            engine: EngineConfig::default(),
            feedback: Feedback::default(),
            hotkeys: crate::hotkeys::Hotkeys::default(),
            theme: Theme::default(),
            language: LanguageSetting::default(),
            onboarded: false,
        }
    }
}

pub fn track_names(language: Language) -> DefaultTrackNames {
    DefaultTrackNames {
        mix: i18n::text(language, "tracks.mix"),
        microphone: i18n::text(language, "tracks.microphone"),
        game: i18n::text(language, "tracks.game"),
    }
}

fn forget_default_names(audio: &mut AudioSettings) {
    let defaults = [Language::Fr, Language::En].map(track_names);
    let forget = |label: &mut Option<String>, pick: fn(&DefaultTrackNames) -> &String| {
        if label
            .as_ref()
            .is_some_and(|name| defaults.iter().any(|names| pick(names) == name))
        {
            *label = None;
        }
    };
    forget(&mut audio.full_mix_label, |names| &names.mix);
    forget(&mut audio.microphone_label, |names| &names.microphone);
    forget(&mut audio.game_label, |names| &names.game);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Feedback {
    pub sound: bool,
    pub toast: bool,
}

impl Default for Feedback {
    fn default() -> Self {
        Self {
            sound: true,
            toast: true,
        }
    }
}

#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    pub writable: bool,
    pub problem: Option<Localized>,
}

pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: Settings::default(),
                writable: true,
                problem: None,
            };
        }
        Err(error) => return unreadable(path, &error.to_string()),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => return unreadable(path, &error.to_string()),
    };
    let version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    if version > u64::from(SETTINGS_VERSION) {
        return Loaded {
            settings: Settings {
                onboarded: true,
                ..Settings::default()
            },
            writable: false,
            problem: Some(Localized::new("settings_newer").with("version", version)),
        };
    }
    let onboarded = value.get("onboarded").is_none_or(|flag| flag != false);
    let mut settings = match serde_json::from_value::<Settings>(value) {
        Ok(settings) => settings,
        Err(error) => return unreadable(path, &error.to_string()),
    };
    if version < 2 {
        forget_default_names(&mut settings.engine.audio);
    }
    if version < 3 && settings.engine.audio.desktop_in_full_mix {
        settings.engine.audio.desktop_excludes_palaclips = false;
    }
    if let Err(error) = settings.engine.validate() {
        return unreadable(path, &error.to_string());
    }
    Loaded {
        settings: Settings {
            schema_version: SETTINGS_VERSION,
            onboarded,
            ..settings
        },
        writable: true,
        problem: None,
    }
}

fn unreadable(path: &Path, reason: &str) -> Loaded {
    let backup = path.with_extension("json.bak");
    let kept = std::fs::rename(path, &backup).is_ok();
    tracing::warn!(reason, path = %path.display(), "réglages illisibles, réinitialisés");
    Loaded {
        settings: Settings {
            onboarded: true,
            ..Settings::default()
        },
        writable: true,
        problem: Some(if kept {
            Localized::new("settings_unreadable_kept")
                .with("detail", reason)
                .with("backup", backup.display().to_string())
        } else {
            Localized::new("settings_unreadable").with("detail", reason)
        }),
    }
}

fn merge_patch(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch))
            if !target.contains_key("kind") && !patch.contains_key("kind") =>
        {
            for (key, value) in patch {
                merge_patch(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, patch) => *target = patch,
    }
}

pub fn patched(config: &EngineConfig, patch: Value) -> serde_json::Result<EngineConfig> {
    let mut value = serde_json::to_value(config)?;
    merge_patch(&mut value, patch);
    serde_json::from_value(value)
}

pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary: PathBuf = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("palaclips-settings-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir.join("settings.json")
    }

    #[test]
    fn patch_keeps_fields_changed_elsewhere() {
        let mut current = EngineConfig::default();
        current.replay = palaclips_engine::config::ReplaySettings {
            duration_secs: 120,
            ..current.replay
        };
        let config =
            patched(&current, serde_json::json!({ "video": { "fps": 120 } })).expect("patch");
        assert_eq!(config.replay.duration_secs, 120);
        assert_eq!(config.video.fps, 120);
        assert_eq!(config.video.bitrate_kbps, current.video.bitrate_kbps);
    }

    #[test]
    fn patch_replaces_tagged_values_whole() {
        let current = EngineConfig {
            capture: palaclips_engine::config::CaptureTarget::Window {
                exe_name: "game.exe".into(),
                title_hint: Some("Game".into()),
            },
            ..EngineConfig::default()
        };
        let config = patched(
            &current,
            serde_json::json!({ "capture": { "kind": "primary_monitor" } }),
        )
        .expect("patch");
        assert_eq!(
            config.capture,
            palaclips_engine::config::CaptureTarget::PrimaryMonitor
        );
        let saved = serde_json::to_value(&config).expect("json");
        assert!(saved["capture"].get("exe_name").is_none());
    }

    #[test]
    fn restart_cases_match_the_engine() {
        let cases: Value = serde_json::from_str(include_str!("../../src/lib/restart-cases.json"))
            .expect("restart cases");
        let defaults = serde_json::to_value(EngineConfig::default()).expect("json");
        assert_eq!(
            cases["base"], defaults,
            "restart-cases.json base must be EngineConfig::default(): {defaults}"
        );
        for case in cases["cases"].as_array().expect("cases") {
            let before = patched(&EngineConfig::default(), case["before"].clone())
                .unwrap_or_else(|_| EngineConfig::default());
            let after = patched(&before, case["after"].clone()).expect("after");
            assert_eq!(
                palaclips_engine::engine::needs_restart(&before, &after),
                case["restarts"].as_bool().expect("restarts"),
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn missing_file_gives_writable_defaults() {
        let loaded = load(&scratch("missing"));
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.writable);
        assert!(loaded.problem.is_none());
    }

    const SETTINGS_1_2: &str = r#"{
  "engine": {
    "audio": {
      "app_tracks": [{ "enabled": true, "exe_name": "vesktop.exe", "label": "Discord", "sync_offset_ms": 0 }],
      "auto_game_track": true, "desktop_in_full_mix": false, "full_mix_label": null,
      "full_mix_sync_offset_ms": -103, "game_exe": null, "game_label": null, "game_sync_offset_ms": 0,
      "max_app_tracks": 6, "microphone_device_id": null, "microphone_gain_db": 0.0,
      "microphone_in_full_mix": true, "microphone_label": null, "microphone_sync_offset_ms": -448,
      "output_device_id": null, "record_full_mix": true, "record_microphone": true
    },
    "capture": { "kind": "primary_monitor" },
    "record": { "include_replay": false, "split": { "kind": "never" } },
    "replay": { "after_secs": 10, "arm_on_startup": true, "duration_secs": 120, "storage": "ram" },
    "schema_version": 1,
    "storage": { "clips_dir": null, "game_names": { "rocketleague.exe": "Rocket League" }, "quota_gb": null },
    "video": {
      "bitrate_kbps": 20000, "capture_cursor": true, "codec": "h264", "draw_border": false,
      "encoder": "auto", "fps": 120, "gpu_priority": "capture", "keyframe_interval_secs": 1.0,
      "quality": "light", "resolution": { "height": 1080, "kind": "height" }
    }
  },
  "feedback": { "sound": true, "toast": true },
  "hotkeys": { "save_replay": "Ctrl+F10", "toggle_recording": "Ctrl+Alt+F9", "toggle_replay": "Ctrl+Alt+F10" },
  "language": "system", "onboarded": true, "schema_version": 2, "theme": "system"
}"#;

    #[test]
    fn settings_from_1_2_record_in_the_replay_quality() {
        let path = scratch("from-1-2");
        std::fs::write(&path, SETTINGS_1_2).expect("write");
        let loaded = load(&path);
        assert!(loaded.problem.is_none());
        let engine = &loaded.settings.engine;
        assert_eq!(engine.record.video, None);
        assert!(!engine.records_separately());
        assert_eq!(engine.record_video(), engine.video);
        assert_eq!(engine.video.fps, 120);
        assert_eq!(engine.replay.after_secs, 10);
        assert_eq!(engine.audio.app_tracks.len(), 1);
        assert_eq!(
            loaded.settings.hotkeys.save_replay.as_deref(),
            Some("Ctrl+F10")
        );
    }

    #[test]
    fn a_desktop_mix_set_before_version_three_keeps_its_output_and_calibration() {
        let path = scratch("desktop-v2");
        std::fs::write(
            &path,
            SETTINGS_1_2.replace(
                r#""desktop_in_full_mix": false"#,
                r#""desktop_in_full_mix": true"#,
            ),
        )
        .expect("write");
        let audio = load(&path).settings.engine.audio;
        assert!(audio.desktop_in_full_mix);
        assert!(!audio.desktop_excludes_palaclips);
        assert_eq!(audio.full_mix_sync_offset_ms, -103);
    }

    #[test]
    fn the_desktop_mix_leaves_palaclips_sounds_out_unless_chosen_otherwise() {
        assert!(Settings::default().engine.audio.desktop_excludes_palaclips);
        let path = scratch("desktop-sum-v2");
        std::fs::write(&path, SETTINGS_1_2).expect("write");
        assert!(load(&path).settings.engine.audio.desktop_excludes_palaclips);
        let path = scratch("desktop-v3");
        std::fs::write(
            &path,
            r#"{ "schema_version": 3, "engine": { "audio": { "desktop_in_full_mix": true } } }"#,
        )
        .expect("write");
        assert!(load(&path).settings.engine.audio.desktop_excludes_palaclips);
    }

    #[test]
    fn max_app_tracks_is_ignored_and_dropped_on_save() {
        let path = scratch("max-app-tracks");
        std::fs::write(&path, SETTINGS_1_2).expect("write");
        let loaded = load(&path);
        assert!(loaded.problem.is_none());
        save(&path, &loaded.settings).expect("save");
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(!text.contains("max_app_tracks"));
    }

    #[test]
    fn saved_settings_round_trip() {
        let path = scratch("round-trip");
        let mut settings = Settings::default();
        settings.engine.replay.duration_secs = 120;
        save(&path, &settings).expect("save");
        let loaded = load(&path);
        assert_eq!(loaded.settings, settings);
        assert!(loaded.problem.is_none());
    }

    #[test]
    fn corrupt_file_is_backed_up_and_reset() {
        let path = scratch("corrupt");
        std::fs::write(&path, "{ pas du json").expect("write");
        let loaded = load(&path);
        assert_eq!(loaded.settings.engine, EngineConfig::default());
        assert!(loaded.settings.onboarded);
        assert!(loaded.writable);
        assert!(loaded.problem.is_some());
        assert!(path.with_extension("json.bak").exists());
        assert!(!path.exists());
    }

    #[test]
    fn invalid_values_are_reset() {
        let path = scratch("invalid");
        let mut settings = Settings::default();
        settings.engine.replay.duration_secs = 1;
        std::fs::write(&path, serde_json::to_string(&settings).expect("json")).expect("write");
        let loaded = load(&path);
        assert_eq!(loaded.settings.engine, EngineConfig::default());
        assert!(loaded.problem.is_some());
    }

    #[test]
    fn only_a_first_launch_needs_onboarding() {
        assert!(!load(&scratch("first-launch")).settings.onboarded);
        let path = scratch("before-onboarding");
        std::fs::write(&path, r#"{ "schema_version": 2, "engine": {} }"#).expect("write");
        assert!(load(&path).settings.onboarded);
        std::fs::write(&path, r#"{ "schema_version": 2, "onboarded": false }"#).expect("write");
        assert!(!load(&path).settings.onboarded);
    }

    #[test]
    fn newer_format_is_left_untouched() {
        let path = scratch("newer");
        let text = r#"{ "schema_version": 99, "engine": {} }"#;
        std::fs::write(&path, text).expect("write");
        let loaded = load(&path);
        assert!(!loaded.writable);
        assert!(loaded.problem.is_some());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
    }

    #[test]
    fn default_track_names_follow_the_language() {
        assert_eq!(track_names(Language::Fr).mix, "Mix complet");
        let english = track_names(Language::En);
        assert_eq!(english.mix, "Full mix");
        assert_eq!(english.microphone, "Mic");
        assert_eq!(english.game, "Game");
    }

    #[test]
    fn version_one_names_equal_to_a_default_become_default_again() {
        let path = scratch("migrate-names");
        std::fs::write(
            &path,
            r#"{ "schema_version": 1, "engine": { "audio": {
                "full_mix_label": "Mix complet", "microphone_label": "Mic", "game_label": "Partie"
            } } }"#,
        )
        .expect("write");
        let audio = load(&path).settings.engine.audio;
        assert_eq!(audio.full_mix_label, None);
        assert_eq!(audio.microphone_label, None);
        assert_eq!(audio.game_label.as_deref(), Some("Partie"));
    }

    #[test]
    fn a_default_name_chosen_after_the_migration_is_kept() {
        let path = scratch("keep-names");
        std::fs::write(
            &path,
            r#"{ "schema_version": 2, "engine": { "audio": { "game_label": "Jeu" } } }"#,
        )
        .expect("write");
        assert_eq!(
            load(&path).settings.engine.audio.game_label.as_deref(),
            Some("Jeu")
        );
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let path = scratch("partial");
        std::fs::write(
            &path,
            r#"{ "schema_version": 1, "engine": { "replay": { "duration_secs": 30 } } }"#,
        )
        .expect("write");
        let loaded = load(&path);
        assert_eq!(loaded.settings.engine.replay.duration_secs, 30);
        assert_eq!(loaded.settings.engine.video, EngineConfig::default().video);
    }
}
