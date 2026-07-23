use serde::{Deserialize, Serialize};

/// Transcription language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ru,
    En,
    Zh,
    Auto,
}

impl Language {
    pub fn title(&self) -> &'static str {
        match self {
            Language::Ru => "Russian / ru",
            Language::En => "English / en",
            Language::Zh => "Chinese / zh",
            Language::Auto => "Auto",
        }
    }

    pub fn title_for(&self, ui_language: UiLanguage) -> &'static str {
        match (self, ui_language) {
            (Language::Ru, UiLanguage::Ru) => "Русский / ru",
            (Language::Ru, UiLanguage::Zh) => "俄语 / ru",
            (Language::Ru, _) => "Russian / ru",
            (Language::En, UiLanguage::Ru) => "Английский / en",
            (Language::En, UiLanguage::Zh) => "英语 / en",
            (Language::En, _) => "English / en",
            (Language::Zh, UiLanguage::Ru) => "Китайский / zh",
            (Language::Zh, UiLanguage::Zh) => "中文 / zh",
            (Language::Zh, _) => "Chinese / zh",
            (Language::Auto, UiLanguage::Ru) => "Авто",
            (Language::Auto, UiLanguage::Zh) => "自动",
            (Language::Auto, _) => "Auto",
        }
    }

    /// BCP-47 code for the Whisper API, or None for auto-detect.
    pub fn api_value(&self) -> Option<&'static str> {
        match self {
            Language::Ru => Some("ru"),
            Language::En => Some("en"),
            Language::Zh => Some("zh"),
            Language::Auto => None,
        }
    }

    /// Locale string for whisper-rs / cpal.
    pub fn whisper_lang(&self) -> &'static str {
        match self {
            Language::Ru => "ru",
            Language::En => "en",
            Language::Zh => "zh",
            Language::Auto => "auto",
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::Ru
    }
}

/// Language used by the application UI. This is intentionally separate from
/// `Language`, which controls the speech-recognition request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    En,
    Ru,
    Zh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiText {
    Settings,
    Endpoint,
    ApiKey,
    ApplicationLanguage,
    RecordingDelay,
    PreviewHideDelay,
    Immediately,
    SpeechLanguage,
    Model,
    Auto,
    RefreshModels,
    RealtimePreview,
    Cadence,
    Hotkey,
    Activation,
    Hold,
    Toggle,
    HoldHint,
    ToggleHint,
    Autostart,
    CenterOverlay,
    WakeServer,
    LocalFallback,
    Permissions,
    Microphone,
    Accessibility,
    RecordWindow,
    Diagnostics,
    OpenLogs,
    RevealLogs,
    EditConfig,
    SttEngine,
    Status,
    DownloadLocalModel,
    RedownloadLocalModel,
    OpenModelsFolder,
    Quit,
    NoModel,
    MacOnly,
}

impl UiLanguage {
    pub fn all_cases() -> Vec<UiLanguage> {
        vec![UiLanguage::En, UiLanguage::Ru, UiLanguage::Zh]
    }

    pub fn code(self) -> &'static str {
        match self {
            UiLanguage::En => "en",
            UiLanguage::Ru => "ru",
            UiLanguage::Zh => "zh",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            UiLanguage::En => "English",
            UiLanguage::Ru => "Русский",
            UiLanguage::Zh => "中文",
        }
    }

    /// Convert a BCP-47/Unix locale (for example `ru_RU.UTF-8`) to a UI
    /// language. English is the safe fallback for unsupported locales.
    pub fn from_locale(locale: &str) -> UiLanguage {
        let normalized = locale.to_ascii_lowercase();
        if normalized.starts_with("ru") {
            UiLanguage::Ru
        } else if normalized.starts_with("zh") || normalized.starts_with("cn") {
            UiLanguage::Zh
        } else {
            UiLanguage::En
        }
    }

    /// Best-effort system locale for the native tray before the webview has
    /// reported `navigator.language`.
    pub fn system() -> UiLanguage {
        ["LC_ALL", "LANGUAGE", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|key| std::env::var(key).ok())
            .find(|value| !value.is_empty())
            .map(|locale| UiLanguage::from_locale(&locale))
            .unwrap_or(UiLanguage::En)
    }

    pub fn text(self, key: UiText) -> &'static str {
        match self {
            UiLanguage::En => match key {
                UiText::Settings => "Settings",
                UiText::Endpoint => "Endpoint",
                UiText::ApiKey => "API Key",
                UiText::ApplicationLanguage => "Application language",
                UiText::RecordingDelay => "Recording delay",
                UiText::PreviewHideDelay => "Preview hide delay",
                UiText::Immediately => "immediately",
                UiText::SpeechLanguage => "Speech language",
                UiText::Model => "Model",
                UiText::Auto => "Auto",
                UiText::RefreshModels => "Refresh models",
                UiText::RealtimePreview => "Realtime preview",
                UiText::Cadence => "Cadence",
                UiText::Hotkey => "Hotkey",
                UiText::Activation => "Activation",
                UiText::Hold => "Hold",
                UiText::Toggle => "Toggle",
                UiText::HoldHint => "(press to start, release to stop)",
                UiText::ToggleHint => "(press to start, press again to stop)",
                UiText::Autostart => "Autostart",
                UiText::CenterOverlay => "Centre overlay on screen",
                UiText::WakeServer => "Wake server on dictation start",
                UiText::LocalFallback => "Local fallback on server failure",
                UiText::Permissions => "Permissions",
                UiText::Microphone => "Mic",
                UiText::Accessibility => "Accessibility",
                UiText::RecordWindow => "Record window",
                UiText::Diagnostics => "Diagnostics",
                UiText::OpenLogs => "Open Logs",
                UiText::RevealLogs => "Reveal in Finder",
                UiText::EditConfig => "Edit Config",
                UiText::SttEngine => "STT Engine",
                UiText::Status => "Status",
                UiText::DownloadLocalModel => "Download local model",
                UiText::RedownloadLocalModel => "Re-download local model",
                UiText::OpenModelsFolder => "Open Models Folder",
                UiText::Quit => "Quit",
                UiText::NoModel => "no model",
                UiText::MacOnly => "macOS only",
            },
            UiLanguage::Ru => match key {
                UiText::Settings => "Настройки",
                UiText::Endpoint => "Сервер",
                UiText::ApiKey => "API-ключ",
                UiText::ApplicationLanguage => "Язык приложения",
                UiText::RecordingDelay => "Задержка записи",
                UiText::PreviewHideDelay => "Скрывать результат через",
                UiText::Immediately => "сразу",
                UiText::SpeechLanguage => "Язык распознавания",
                UiText::Model => "Модель",
                UiText::Auto => "Авто",
                UiText::RefreshModels => "Обновить модели",
                UiText::RealtimePreview => "Предпросмотр в реальном времени",
                UiText::Cadence => "Интервал",
                UiText::Hotkey => "Горячая клавиша",
                UiText::Activation => "Режим активации",
                UiText::Hold => "Удержание",
                UiText::Toggle => "Переключатель",
                UiText::HoldHint => "(нажать для старта, отпустить для остановки)",
                UiText::ToggleHint => "(нажать для старта, нажать ещё раз для остановки)",
                UiText::Autostart => "Автозапуск",
                UiText::CenterOverlay => "Центрировать индикатор",
                UiText::WakeServer => "Прогревать сервер при начале диктовки",
                UiText::LocalFallback => "Локальный fallback при ошибке сервера",
                UiText::Permissions => "Разрешения",
                UiText::Microphone => "Микрофон",
                UiText::Accessibility => "Доступность",
                UiText::RecordWindow => "Окно записи",
                UiText::Diagnostics => "Диагностика",
                UiText::OpenLogs => "Открыть логи",
                UiText::RevealLogs => "Показать в Finder",
                UiText::EditConfig => "Изменить конфигурацию",
                UiText::SttEngine => "Движок распознавания",
                UiText::Status => "Статус",
                UiText::DownloadLocalModel => "Скачать локальную модель",
                UiText::RedownloadLocalModel => "Скачать локальную модель заново",
                UiText::OpenModelsFolder => "Открыть папку моделей",
                UiText::Quit => "Выйти",
                UiText::NoModel => "нет модели",
                UiText::MacOnly => "только macOS",
            },
            UiLanguage::Zh => match key {
                UiText::Settings => "设置",
                UiText::Endpoint => "服务器",
                UiText::ApiKey => "API 密钥",
                UiText::ApplicationLanguage => "应用语言",
                UiText::RecordingDelay => "录音延迟",
                UiText::PreviewHideDelay => "结果隐藏延迟",
                UiText::Immediately => "立即",
                UiText::SpeechLanguage => "识别语言",
                UiText::Model => "模型",
                UiText::Auto => "自动",
                UiText::RefreshModels => "刷新模型",
                UiText::RealtimePreview => "实时预览",
                UiText::Cadence => "间隔",
                UiText::Hotkey => "快捷键",
                UiText::Activation => "激活模式",
                UiText::Hold => "按住",
                UiText::Toggle => "切换",
                UiText::HoldHint => "（按下开始，松开停止）",
                UiText::ToggleHint => "（按下开始，再次按下停止）",
                UiText::Autostart => "开机启动",
                UiText::CenterOverlay => "将提示框置中",
                UiText::WakeServer => "开始听写时预热服务器",
                UiText::LocalFallback => "服务器失败时使用本地识别",
                UiText::Permissions => "权限",
                UiText::Microphone => "麦克风",
                UiText::Accessibility => "辅助功能",
                UiText::RecordWindow => "录音窗口",
                UiText::Diagnostics => "诊断",
                UiText::OpenLogs => "打开日志",
                UiText::RevealLogs => "在 Finder 中显示",
                UiText::EditConfig => "编辑配置",
                UiText::SttEngine => "识别引擎",
                UiText::Status => "状态",
                UiText::DownloadLocalModel => "下载本地模型",
                UiText::RedownloadLocalModel => "重新下载本地模型",
                UiText::OpenModelsFolder => "打开模型文件夹",
                UiText::Quit => "退出",
                UiText::NoModel => "没有模型",
                UiText::MacOnly => "仅 macOS",
            },
        }
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

    pub fn title_for(&self, ui_language: UiLanguage) -> &'static str {
        match (self, ui_language) {
            (HotkeyKind::Fn, UiLanguage::Ru) => "Fn (Globe 🌐)",
            (HotkeyKind::Fn, UiLanguage::Zh) => "Fn（地球 🌐）",
            (HotkeyKind::Fn, _) => "Fn (Globe 🌐)",
            (HotkeyKind::RightOption, UiLanguage::Ru) => "Правая ⌥ Option",
            (HotkeyKind::RightOption, UiLanguage::Zh) => "右侧 ⌥ Option",
            (HotkeyKind::RightOption, _) => "Right ⌥ Option",
            (HotkeyKind::RightControl, UiLanguage::Ru) => "Правая ⌃ Control",
            (HotkeyKind::RightControl, UiLanguage::Zh) => "右侧 ⌃ Control",
            (HotkeyKind::RightControl, _) => "Right ⌃ Control",
            (HotkeyKind::RightCommand, UiLanguage::Ru) => "Правая ⌘ Command",
            (HotkeyKind::RightCommand, UiLanguage::Zh) => "右侧 ⌘ Command",
            (HotkeyKind::RightCommand, _) => "Right ⌘ Command",
            (HotkeyKind::RightShift, UiLanguage::Ru) => "Правая ⇧ Shift",
            (HotkeyKind::RightShift, UiLanguage::Zh) => "右侧 ⇧ Shift",
            (HotkeyKind::RightShift, _) => "Right ⇧ Shift",
            (HotkeyKind::CapsLock, UiLanguage::Ru) => "Caps Lock",
            (HotkeyKind::CapsLock, UiLanguage::Zh) => "Caps Lock",
            (HotkeyKind::CapsLock, _) => "Caps Lock",
            (HotkeyKind::F13, _) => "F13",
            (HotkeyKind::F14, _) => "F14",
            (HotkeyKind::F15, _) => "F15",
        }
    }

    /// The global shortcut string for Tauri.
    /// Note: Modifier-only keys are handled by the Swift helper on macOS,
    /// so this method only returns strings for non-modifier keys (F13-F15).
    pub fn shortcut_str(&self) -> &'static str {
        match self {
            // Modifier-only keys use the Swift helper, not Tauri's global-shortcut
            HotkeyKind::Fn => "",
            HotkeyKind::RightOption => "",
            HotkeyKind::RightControl => "",
            HotkeyKind::RightCommand => "",
            HotkeyKind::RightShift => "",
            HotkeyKind::CapsLock => "",
            // Regular function keys work with Tauri's global-shortcut plugin
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

/// Which STT engine to use. Cascading fallbacks: the order in
/// `AppConfig::engine_order` defines the priority list — first available wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SttEngine {
    /// Remote Whisper-compatible API (OpenAI, Groq, local server…).
    Remote,
    /// Local on-device whisper.cpp (requires downloaded model).
    Local,
    /// Apple built-in Speech framework (macOS only).
    Native,
}

impl SttEngine {
    /// Snake-case id used in tray menu event ids (e.g. "engine_remote").
    pub fn id(self) -> &'static str {
        match self {
            SttEngine::Remote => "engine_remote",
            SttEngine::Local => "engine_local",
            SttEngine::Native => "engine_native",
        }
    }

    /// Human label with an emoji bullet — used in the STT Engine submenu.
    pub fn title(self) -> &'static str {
        match self {
            SttEngine::Remote => "🌐 Remote (Whisper API)",
            SttEngine::Local => "💾 Local (whisper.cpp)",
            SttEngine::Native => "🍎 Native (Apple Speech)",
        }
    }

    pub fn title_for(self, ui_language: UiLanguage) -> &'static str {
        match (self, ui_language) {
            (SttEngine::Remote, UiLanguage::Ru) => "🌐 Удалённый (Whisper API)",
            (SttEngine::Remote, UiLanguage::Zh) => "🌐 远程（Whisper API）",
            (SttEngine::Remote, _) => "🌐 Remote (Whisper API)",
            (SttEngine::Local, UiLanguage::Ru) => "💾 Локальный (whisper.cpp)",
            (SttEngine::Local, UiLanguage::Zh) => "💾 本地（whisper.cpp）",
            (SttEngine::Local, _) => "💾 Local (whisper.cpp)",
            (SttEngine::Native, UiLanguage::Ru) => "🍎 Системный (Apple Speech)",
            (SttEngine::Native, UiLanguage::Zh) => "🍎 原生（Apple Speech）",
            (SttEngine::Native, _) => "🍎 Native (Apple Speech)",
        }
    }

    /// Compact one-word label — used in the status line.
    pub fn short(self) -> &'static str {
        match self {
            SttEngine::Remote => "Remote",
            SttEngine::Local => "Local",
            SttEngine::Native => "Native",
        }
    }

    pub fn short_for(self, ui_language: UiLanguage) -> &'static str {
        match (self, ui_language) {
            (SttEngine::Remote, UiLanguage::Ru) => "Удалённый",
            (SttEngine::Remote, UiLanguage::Zh) => "远程",
            (SttEngine::Remote, _) => "Remote",
            (SttEngine::Local, UiLanguage::Ru) => "Локальный",
            (SttEngine::Local, UiLanguage::Zh) => "本地",
            (SttEngine::Local, _) => "Local",
            (SttEngine::Native, UiLanguage::Ru) => "Системный",
            (SttEngine::Native, UiLanguage::Zh) => "原生",
            (SttEngine::Native, _) => "Native",
        }
    }

    /// All cases, in the canonical UI order (Remote → Local → Native).
    pub fn all_cases() -> Vec<SttEngine> {
        vec![SttEngine::Remote, SttEngine::Local, SttEngine::Native]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_language_maps_supported_system_locales() {
        assert_eq!(UiLanguage::from_locale("ru_RU.UTF-8"), UiLanguage::Ru);
        assert_eq!(UiLanguage::from_locale("zh-CN"), UiLanguage::Zh);
        assert_eq!(UiLanguage::from_locale("en_US.UTF-8"), UiLanguage::En);
    }

    #[test]
    fn ui_language_has_three_native_choices() {
        assert_eq!(
            UiLanguage::all_cases(),
            vec![UiLanguage::En, UiLanguage::Ru, UiLanguage::Zh]
        );
        assert_eq!(UiLanguage::Ru.text(UiText::Settings), "Настройки");
        assert_eq!(UiLanguage::Zh.text(UiText::Settings), "设置");
    }
}
