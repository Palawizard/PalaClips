use std::str::FromStr;

use palaclips_engine::events::Localized;
use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyAction {
    SaveReplay,
    ToggleReplay,
}

impl HotkeyAction {
    pub fn text(self) -> Localized {
        match self {
            Self::SaveReplay => Localized::new("action_save_replay"),
            Self::ToggleReplay => Localized::new("action_toggle_replay"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub save_replay: Option<String>,
    pub toggle_replay: Option<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            save_replay: Some("Alt+F10".into()),
            toggle_replay: Some("Ctrl+Alt+F10".into()),
        }
    }
}

impl Hotkeys {
    pub fn bindings(&self) -> Vec<(HotkeyAction, &str)> {
        [
            (HotkeyAction::SaveReplay, self.save_replay.as_deref()),
            (HotkeyAction::ToggleReplay, self.toggle_replay.as_deref()),
        ]
        .into_iter()
        .filter_map(|(action, keys)| keys.map(|keys| (action, keys)))
        .collect()
    }
}

const SYSTEM_SHORTCUTS: &[(Modifiers, Code)] = &[
    (Modifiers::ALT, Code::F4),
    (Modifiers::ALT, Code::Tab),
    (Modifiers::ALT, Code::Escape),
    (Modifiers::ALT, Code::Space),
    (Modifiers::ALT, Code::Enter),
    (Modifiers::CONTROL, Code::Escape),
    (Modifiers::CONTROL.union(Modifiers::SHIFT), Code::Escape),
    (Modifiers::CONTROL.union(Modifiers::ALT), Code::Delete),
];

fn is_function_key(code: Code) -> bool {
    matches!(
        code,
        Code::F13
            | Code::F14
            | Code::F15
            | Code::F16
            | Code::F17
            | Code::F18
            | Code::F19
            | Code::F20
            | Code::F21
            | Code::F22
            | Code::F23
            | Code::F24
    )
}

fn types_text(code: Code) -> bool {
    let name = format!("{code:?}");
    name.starts_with("Key")
        || name.starts_with("Digit")
        || matches!(
            code,
            Code::Space
                | Code::Minus
                | Code::Equal
                | Code::Comma
                | Code::Period
                | Code::Slash
                | Code::Semicolon
                | Code::Quote
                | Code::Backquote
                | Code::BracketLeft
                | Code::BracketRight
                | Code::Backslash
                | Code::IntlBackslash
        )
}

pub fn parse(keys: &str) -> Result<Shortcut, Localized> {
    let refused = |text: Localized| Err(text.with("keys", keys));
    let Ok(shortcut) = Shortcut::from_str(keys.trim()) else {
        return refused(Localized::new("hotkey_unrecognized"));
    };
    let mods = shortcut.mods;
    let key = shortcut.key;
    if key == Code::Tab {
        return refused(Localized::new("hotkey_tab"));
    }
    if mods.contains(Modifiers::SUPER) {
        return refused(Localized::new("hotkey_windows_key"));
    }
    if SYSTEM_SHORTCUTS
        .iter()
        .any(|(system_mods, system_key)| *system_mods == mods && *system_key == key)
    {
        return refused(Localized::new("hotkey_system"));
    }
    if mods.is_empty() && !is_function_key(key) {
        return refused(Localized::new("hotkey_needs_modifier"));
    }
    if mods == Modifiers::SHIFT && types_text(key) {
        return refused(Localized::new("hotkey_shift_types"));
    }
    Ok(shortcut)
}

pub fn canonical(shortcut: &Shortcut) -> String {
    let mut parts = Vec::new();
    if shortcut.mods.contains(Modifiers::CONTROL) {
        parts.push("Ctrl".to_owned());
    }
    if shortcut.mods.contains(Modifiers::ALT) {
        parts.push("Alt".to_owned());
    }
    if shortcut.mods.contains(Modifiers::SHIFT) {
        parts.push("Shift".to_owned());
    }
    let key = format!("{:?}", shortcut.key);
    let key = key
        .strip_prefix("Key")
        .or_else(|| key.strip_prefix("Digit"))
        .unwrap_or(&key)
        .to_owned();
    parts.push(key);
    parts.join("+")
}

pub fn validate(hotkeys: &Hotkeys) -> Result<Vec<(HotkeyAction, Shortcut)>, Localized> {
    let mut parsed: Vec<(HotkeyAction, Shortcut)> = Vec::new();
    for (action, keys) in hotkeys.bindings() {
        let shortcut = parse(keys)?;
        if let Some((other, _)) = parsed.iter().find(|(_, taken)| *taken == shortcut) {
            return Err(Localized::new("hotkey_duplicate")
                .with("keys", keys)
                .with("action", other.text()));
        }
        parsed.push((action, shortcut));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkeys_are_valid() {
        let parsed = validate(&Hotkeys::default()).expect("defaults");
        assert_eq!(parsed.len(), 2);
        assert_eq!(canonical(&parsed[0].1), "Alt+F10");
        assert_eq!(canonical(&parsed[1].1), "Ctrl+Alt+F10");
    }

    #[test]
    fn canonical_names_parse_back_to_the_same_shortcut() {
        for keys in ["ctrl+shift+s", "Alt+1", "Ctrl+Alt+F9", "F13"] {
            let shortcut = parse(keys).expect(keys);
            assert_eq!(parse(&canonical(&shortcut)).expect("canonical"), shortcut);
        }
        assert_eq!(
            canonical(&parse("ctrl+shift+s").expect("parse")),
            "Ctrl+Shift+S"
        );
    }

    #[test]
    fn reserved_and_game_breaking_shortcuts_are_refused() {
        for keys in [
            "Alt+F4",
            "Alt+Tab",
            "Ctrl+Shift+Escape",
            "Super+G",
            "G",
            "F10",
            "Shift+R",
            "Alt+Enter",
        ] {
            assert!(parse(keys).is_err(), "{keys}");
        }
        assert!(parse("Ctrl+Shift+R").is_ok());
    }

    #[test]
    fn tab_stays_free_for_keyboard_navigation() {
        for keys in ["Shift+Tab", "Ctrl+Tab", "Ctrl+Alt+Tab"] {
            assert!(parse(keys).is_err(), "{keys}");
        }
    }

    #[test]
    fn unreadable_shortcuts_are_refused() {
        assert!(parse("Alt+").is_err());
        assert!(parse("Banane+F10").is_err());
    }

    #[test]
    fn one_shortcut_cannot_serve_two_actions() {
        let hotkeys = Hotkeys {
            save_replay: Some("Alt+F10".into()),
            toggle_replay: Some("alt+f10".into()),
        };
        let error = validate(&hotkeys).expect_err("duplicate");
        assert_eq!(error.code, "hotkey_duplicate");
        assert_eq!(error.params["action"]["code"], "action_save_replay");
    }

    #[test]
    fn an_action_can_have_no_shortcut() {
        let hotkeys = Hotkeys {
            save_replay: Some("Alt+F10".into()),
            toggle_replay: None,
        };
        assert_eq!(validate(&hotkeys).expect("valid").len(), 1);
    }
}
