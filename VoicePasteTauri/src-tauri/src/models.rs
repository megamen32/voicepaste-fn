use serde::{Deserialize, Serialize};

/// Transcription language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ru,
    En,
    Auto,
}

impl Language {
    pub fn title(&self) -> &'static str {
        match self {
            Language::Ru => "Russian / ru",
            Language::En => "English / en",
            Language::Auto => "Auto",
        }
    }

    /// BCP-47 code for the Whisper API, or None for auto-detect.
    pub fn api_value(&self) -> Option<&'static str> {
        match self {
            Language::Ru => Some("ru"),
            Language::En => Some("en"),
            Language::Auto => None,
        }
    }

    /// Locale string for whisper-rs / cpal.
    pub fn whisper_lang(&self) -> &'static str {
        match self {
            Language::Ru => "ru",
            Language::En => "en",
            Language::Auto => "auto",
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::Ru
    }
}

/// Which physical key triggers dictation.
/// Matches the macOS Swift version exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyKind {
    /// Fn / Globe key (default on macOS)
    Fn,
    /// Right Option (⌥)
    RightOption,
    /// Right Control (⌃)
    RightControl,
    /// Right Command (⌘)
    RightCommand,
    /// Right Shift (⇧)
    RightShift,
    /// Caps Lock
    CapsLock,
    /// F13 (external keyboards)
    F13,
    /// F14 (external keyboards)
    F14,
    /// F15 (external keyboards)
    F15,
}

impl HotkeyKind {
    pub fn title(&self) -> &'static str {
        match self {
            HotkeyKind::Fn => "Fn (Globe 🌐)",
            HotkeyKind::RightOption => "Right ⌥ Option",
            HotkeyKind::RightControl => "Right ⌃ Control",
            HotkeyKind::RightCommand => "Right ⌘ Command",
            HotkeyKind::RightShift => "Right ⇧ Shift",
            HotkeyKind::CapsLock => "Caps Lock",
            HotkeyKind::F13 => "F13",
            HotkeyKind::F14 => "F14",
            HotkeyKind::F15 => "F15",
        }
    }

    /// The global shortcut string for Tauri.
    pub fn shortcut_str(&self) -> &'static str {
        match self {
            // Fn/Globe is not directly supported by Tauri global-shortcut.
            // We register "Fn" as a placeholder — actual detection needs
            // a CGEvent tap on macOS. For now, map to a rarely-used shortcut.
            HotkeyKind::Fn => "F16",
            HotkeyKind::RightOption => "AltRight",
            HotkeyKind::RightControl => "ControlRight",
            HotkeyKind::RightCommand => "MetaRight",
            HotkeyKind::RightShift => "ShiftRight",
            HotkeyKind::CapsLock => "CapsLock",
            HotkeyKind::F13 => "F13",
            HotkeyKind::F14 => "F14",
            HotkeyKind::F15 => "F15",
        }
    }

    /// All cases for iteration (matches macOS Swift CaseIterable).
    pub fn all_cases() -> Vec<HotkeyKind> {
        vec![
            HotkeyKind::Fn,
            HotkeyKind::RightOption,
            HotkeyKind::RightControl,
            HotkeyKind::RightCommand,
            HotkeyKind::RightShift,
            HotkeyKind::CapsLock,
            HotkeyKind::F13,
            HotkeyKind::F14,
            HotkeyKind::F15,
        ]
    }
}

impl Default for HotkeyKind {
    fn default() -> Self {
        HotkeyKind::Fn
    }
}

/// How the hotkey triggers recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivationMode {
    /// Press to start, release to stop.
    Hold,
    /// First press starts, second press stops.
    Toggle,
}

impl ActivationMode {
    pub fn title(&self) -> &'static str {
        match self {
            ActivationMode::Hold => "Hold (press to start, release to stop)",
            ActivationMode::Toggle => "Toggle (press to start, press again to stop)",
        }
    }
}

impl Default for ActivationMode {
    fn default() -> Self {
        ActivationMode::Hold
    }
}
