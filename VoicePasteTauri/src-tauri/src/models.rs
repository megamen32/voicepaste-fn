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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyKind {
    /// Right Alt (default on Windows/Linux)
    RightAlt,
    /// Scroll Lock
    ScrollLock,
    /// Caps Lock
    CapsLock,
    /// Insert key
    Insert,
    /// F13 (macOS external keyboards)
    F13,
    /// F14
    F14,
    /// F15
    F15,
    /// Right Shift
    RightShift,
    /// Right Control
    RightControl,
}

impl HotkeyKind {
    pub fn title(&self) -> &'static str {
        match self {
            HotkeyKind::RightAlt => "Right Alt",
            HotkeyKind::ScrollLock => "Scroll Lock",
            HotkeyKind::CapsLock => "Caps Lock",
            HotkeyKind::Insert => "Insert",
            HotkeyKind::F13 => "F13",
            HotkeyKind::F14 => "F14",
            HotkeyKind::F15 => "F15",
            HotkeyKind::RightShift => "Right Shift",
            HotkeyKind::RightControl => "Right Ctrl",
        }
    }

    /// The global shortcut string for Tauri (e.g. "AltRight", "ScrollLock").
    pub fn shortcut_str(&self) -> &'static str {
        match self {
            HotkeyKind::RightAlt => "AltRight",
            HotkeyKind::ScrollLock => "ScrollLock",
            HotkeyKind::CapsLock => "CapsLock",
            HotkeyKind::Insert => "Insert",
            HotkeyKind::F13 => "F13",
            HotkeyKind::F14 => "F14",
            HotkeyKind::F15 => "F15",
            HotkeyKind::RightShift => "ShiftRight",
            HotkeyKind::RightControl => "ControlRight",
        }
    }
}

impl Default for HotkeyKind {
    fn default() -> Self {
        HotkeyKind::RightAlt
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
