use std::sync::LazyLock;

use palaclips_engine::events::Localized;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageSetting {
    #[default]
    System,
    Fr,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Fr,
    En,
}

impl LanguageSetting {
    pub fn resolve(self) -> Language {
        match self {
            Self::System => system_language(),
            Self::Fr => Language::Fr,
            Self::En => Language::En,
        }
    }
}

const PRIMARY_LANGUAGE_MASK: u16 = 0x3ff;
const LANG_FRENCH: u16 = 0x0c;

pub fn system_language() -> Language {
    // SAFETY: GetUserDefaultUILanguage takes no argument and only reads the user's profile.
    let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    if id & PRIMARY_LANGUAGE_MASK == LANG_FRENCH {
        Language::Fr
    } else {
        Language::En
    }
}

static FR: LazyLock<Value> = LazyLock::new(|| parse(include_str!("../../src/locales/fr.json")));
static EN: LazyLock<Value> = LazyLock::new(|| parse(include_str!("../../src/locales/en.json")));

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

pub fn text(language: Language, key: &str) -> String {
    let dictionary = match language {
        Language::Fr => &*FR,
        Language::En => &*EN,
    };
    key.split('.')
        .try_fold(dictionary, |node, part| node.get(part))
        .and_then(Value::as_str)
        .map_or_else(|| key.to_owned(), str::to_owned)
}

pub fn render(language: Language, message: &Localized) -> String {
    render_code(language, message.code, &message.params)
}

fn render_code(language: Language, code: &str, params: &Map<String, Value>) -> String {
    let mut rendered = text(language, &format!("messages.{code}"));
    for (name, value) in params {
        let shown = match value {
            Value::String(value) => value.clone(),
            Value::Object(nested) => match (
                nested.get("code").and_then(Value::as_str),
                nested.get("params").and_then(Value::as_object),
            ) {
                (Some(code), Some(params)) => render_code(language, code, params),
                _ => value.to_string(),
            },
            other => other.to_string(),
        };
        rendered = rendered.replace(&format!("{{{name}}}"), &shown);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_dictionaries_parse() {
        assert!(FR.is_object());
        assert!(EN.is_object());
    }

    #[test]
    fn the_exclusive_fullscreen_alert_carries_the_minecraft_advice() {
        let (advice, _) = palaclips_engine::capture::exclusive_fullscreen_advice(Some("javaw.exe"));
        let message = Localized::new("capture_exclusive_fullscreen").with("advice", advice);
        assert!(render(Language::Fr, &message).ends_with(
            "la vidéo reste figée. Minecraft Java : quitte le plein écran (F11) et agrandis la fenêtre, ou installe un mod de plein écran sans bordure comme Borderless Fullscreen."
        ));
        assert!(render(Language::En, &message).ends_with(
            "so the video freezes. Minecraft Java: leave fullscreen (F11) and maximize the window, or install a borderless fullscreen mod such as Borderless Fullscreen."
        ));
    }

    #[test]
    fn nested_messages_render_in_the_chosen_language() {
        let message = Localized::new("clip_save_failed").with(
            "reason",
            palaclips_engine::EngineError::ReplayNotArmed.localized(),
        );
        assert_eq!(
            render(Language::Fr, &message),
            "Clip non sauvegardé : le replay n'est pas armé."
        );
        assert_eq!(
            render(Language::En, &message),
            "Clip not saved: the replay isn't armed."
        );
    }

    #[test]
    fn numbers_fill_their_placeholders() {
        let message = palaclips_engine::EngineError::ClipTooShort.localized();
        assert!(render(Language::En, &message).contains("less than 3 s"));
    }

    #[test]
    fn a_missing_key_shows_the_key() {
        assert_eq!(text(Language::En, "tray.nothing"), "tray.nothing");
    }
}
