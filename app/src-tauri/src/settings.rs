use std::path::{Path, PathBuf};

use palaclips_engine::EngineConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub engine: EngineConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_VERSION,
            engine: EngineConfig::default(),
        }
    }
}

#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    pub writable: bool,
    pub problem: Option<String>,
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
            settings: Settings::default(),
            writable: false,
            problem: Some(format!(
                "Tes réglages viennent d'une version plus récente de PalaClips (format {version}) : réglages par défaut pour cette session, fichier laissé intact. Mets PalaClips à jour pour les retrouver."
            )),
        };
    }
    let settings = match serde_json::from_value::<Settings>(value) {
        Ok(settings) => settings,
        Err(error) => return unreadable(path, &error.to_string()),
    };
    if let Err(error) = settings.engine.validate() {
        return unreadable(path, &error.to_string());
    }
    Loaded {
        settings: Settings {
            schema_version: SETTINGS_VERSION,
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
        settings: Settings::default(),
        writable: true,
        problem: Some(if kept {
            format!(
                "Tes réglages étaient illisibles ({reason}) : ils ont été réinitialisés. L'ancien fichier est gardé dans {}.",
                backup.display()
            )
        } else {
            format!("Tes réglages étaient illisibles ({reason}) : ils ont été réinitialisés.")
        }),
    }
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
    fn missing_file_gives_writable_defaults() {
        let loaded = load(&scratch("missing"));
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.writable);
        assert!(loaded.problem.is_none());
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
        assert_eq!(loaded.settings, Settings::default());
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
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.problem.is_some());
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
