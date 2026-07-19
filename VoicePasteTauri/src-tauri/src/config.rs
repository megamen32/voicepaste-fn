use crate::models::{ActivationMode, HotkeyKind, Language};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Default Whisper endpoint.
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "whisper-1";

/// All application settings, persisted to JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub model: String,
    pub language: Language,
    pub realtime_preview: bool,
    pub recording_delay: f64,
    pub hide_delay: f64,
    pub hotkey: HotkeyKind,
    pub activation_mode: ActivationMode,
    pub overlay_centered: bool,
    pub wake_server_on_start: bool,
    pub realtime_chunk_interval: f64,
    pub local_fallback: bool,
    pub autostart: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            language: Language::default(),
            realtime_preview: false,
            recording_delay: 0.20,
            hide_delay: 0.8,
            hotkey: HotkeyKind::default(),
            activation_mode: ActivationMode::default(),
            overlay_centered: false,
            wake_server_on_start: true,
            realtime_chunk_interval: 5.0,
            local_fallback: false,
            autostart: false,
        }
    }
}

impl AppConfig {
    /// Clamped recording delay (0.10 – 2.00s).
    pub fn recording_delay_clamped(&self) -> f64 {
        self.recording_delay.clamp(0.10, 2.0)
    }

    /// Clamped hide delay (0.0 – 5.0s).
    pub fn hide_delay_clamped(&self) -> f64 {
        self.hide_delay.clamp(0.0, 5.0)
    }

    /// Clamped realtime chunk interval (1.0 – 30.0s).
    pub fn realtime_chunk_interval_clamped(&self) -> f64 {
        self.realtime_chunk_interval.clamp(1.0, 30.0)
    }

    /// Effective base URL (env override wins).
    pub fn effective_base_url(&self) -> String {
        if let Ok(env) = std::env::var("OPENAI_BASE_URL") {
            if !env.is_empty() {
                return env;
            }
        }
        self.base_url.clone()
    }

    /// Effective API key (env override wins).
    pub fn effective_api_key(&self) -> String {
        if let Ok(env) = std::env::var("OPENAI_API_KEY") {
            if !env.is_empty() {
                return env;
            }
        }
        self.api_key.clone().unwrap_or_default()
    }

    /// Effective model (env override wins).
    pub fn effective_model(&self) -> String {
        if let Ok(env) = std::env::var("TRANSCRIBE_MODEL") {
            if !env.is_empty() {
                return env;
            }
        }
        self.model.clone()
    }

    /// Masked base URL for display (host only).
    pub fn masked_base_url(&self) -> String {
        self.effective_base_url()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or(&self.effective_base_url())
            .to_string()
    }

    /// Masked API key for display.
    pub fn masked_api_key(&self) -> String {
        let key = self.effective_api_key();
        if key.is_empty() {
            return "(not set)".to_string();
        }
        if key.len() <= 8 {
            return "****".to_string();
        }
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Thread-safe settings manager. Loads from / saves to JSON file.
pub struct AppSettings {
    config: Mutex<AppConfig>,
    path: PathBuf,
}

impl AppSettings {
    /// Get or create the global settings instance.
    pub fn global() -> &'static AppSettings {
        static INSTANCE: OnceLock<AppSettings> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let path = Self::config_path();
            let config = Self::load_from(&path).unwrap_or_default();
            AppSettings {
                config: Mutex::new(config),
                path,
            }
        })
    }

    fn config_path() -> PathBuf {
        // Use OPENAI_CONFIG env var if set (for dev), otherwise platform dirs
        if let Ok(p) = std::env::var("VOICEPASTE_CONFIG") {
            return PathBuf::from(p);
        }
        let dir = directories::ProjectDirs::from("com", "bezrabotnyi", "voicepaste")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| {
                std::env::temp_dir().join("voicepaste-config")
            });
        dir.join("settings.json")
    }

    fn load_from(path: &PathBuf) -> Option<AppConfig> {
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Read current config (cloned).
    pub fn get(&self) -> AppConfig {
        self.config.lock().clone()
    }

    /// Update config and persist.
    pub fn update(&self, mutator: impl FnOnce(&mut AppConfig)) {
        {
            let mut cfg = self.config.lock();
            mutator(&mut cfg);
        }
        self.save();
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let cfg = self.config.lock();
        if let Ok(json) = serde_json::to_string_pretty(&*cfg) {
            let _ = fs::write(&self.path, json);
        }
    }
}
