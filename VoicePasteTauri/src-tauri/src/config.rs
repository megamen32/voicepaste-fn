use crate::history::DEFAULT_RETENTION_DAYS;
use crate::models::{ActivationMode, HotkeyKind, Language, SttEngine, UiLanguage};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Default Whisper endpoint.
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "whisper-1";
const DEFAULT_LOCAL_MODEL: &str = "whisper-base";
const DEFAULT_REMOTE_PROVIDER: &str = "openai";

fn default_local_model() -> String {
    DEFAULT_LOCAL_MODEL.to_string()
}

fn default_remote_provider() -> String {
    DEFAULT_REMOTE_PROVIDER.to_string()
}

/// All application settings, persisted to JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default = "default_local_model")]
    pub local_model: String,
    #[serde(default)]
    pub local_command: Option<String>,
    #[serde(default = "default_remote_provider")]
    pub remote_provider: String,
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
    /// Number of days to keep completed transcriptions. Zero means forever.
    #[serde(default = "default_history_retention_days")]
    pub history_retention_days: u32,
    /// Ordered list of STT engines to try (first available wins).
    /// Defaults to all three (Remote → Local → Native) for graceful degradation.
    #[serde(default = "default_engine_order")]
    pub engine_order: Vec<SttEngine>,
    /// None means first launch / legacy config; the UI initializes it from
    /// navigator.language and persists the user's choice afterwards.
    #[serde(default)]
    pub ui_language: Option<UiLanguage>,
}

/// Default cascading fallback: remote first, then local, then native.
fn default_engine_order() -> Vec<SttEngine> {
    vec![SttEngine::Remote, SttEngine::Local, SttEngine::Native]
}

fn default_history_retention_days() -> u32 {
    DEFAULT_RETENTION_DAYS
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            local_model: DEFAULT_LOCAL_MODEL.to_string(),
            local_command: None,
            remote_provider: DEFAULT_REMOTE_PROVIDER.to_string(),
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
            history_retention_days: DEFAULT_RETENTION_DAYS,
            engine_order: default_engine_order(),
            ui_language: None,
        }
    }
}

/// Pure toggle logic, isolated from state for unit testing.
///
/// Returns the new `engine_order` after toggling `target`:
/// - If `target` is already present → remove it (unless it's the last one,
///   in which case the original order is returned unchanged).
/// - If `target` is not present → append it to the end.
///
/// Callers should compare against the original to detect the "last engine"
/// rejection case.
pub fn toggle_engine(order: &[SttEngine], target: SttEngine) -> Vec<SttEngine> {
    if order.contains(&target) {
        // Disabling: refuse to drop the last engine.
        if order.len() <= 1 {
            return order.to_vec();
        }
        let mut next: Vec<SttEngine> = order.iter().copied().filter(|e| *e != target).collect();
        // Stable order, no duplicates, preserves the rest of the cascade.
        next.dedup();
        next
    } else {
        // Enabling: append to the end of the cascade.
        let mut next: Vec<SttEngine> = order.to_vec();
        next.push(target);
        next
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

    pub fn effective_ui_language(&self) -> UiLanguage {
        self.ui_language.unwrap_or_else(UiLanguage::system)
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
            let path = Self::default_config_path();
            let config = Self::load_from(&path).unwrap_or_default();
            AppSettings {
                config: Mutex::new(config),
                path,
            }
        })
    }

    /// Path to the on-disk JSON config (read-only clone for menu display).
    pub fn config_path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Compute the canonical config path (for the diagnostics menu).
    pub fn default_config_path() -> PathBuf {
        // Use OPENAI_CONFIG env var if set (for dev), otherwise platform dirs
        if let Ok(p) = std::env::var("VOICEPASTE_CONFIG") {
            return PathBuf::from(p);
        }
        let dir = directories::ProjectDirs::from("com", "bezrabotnyi", "voicepaste")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("voicepaste-config"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_order_is_all_three() {
        assert_eq!(
            default_engine_order(),
            vec![SttEngine::Remote, SttEngine::Local, SttEngine::Native]
        );
    }

    #[test]
    fn default_config_includes_all_engines() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.engine_order, default_engine_order());
    }

    #[test]
    fn toggle_engine_appends_when_disabled() {
        let order = vec![SttEngine::Remote];
        assert_eq!(
            toggle_engine(&order, SttEngine::Local),
            vec![SttEngine::Remote, SttEngine::Local]
        );
    }

    #[test]
    fn toggle_engine_appends_to_end_in_cascade() {
        let order = vec![SttEngine::Remote, SttEngine::Native];
        // Native is on; toggling it removes it; toggling Local appends to end.
        let order = toggle_engine(&order, SttEngine::Native);
        assert_eq!(order, vec![SttEngine::Remote]);
        let order = toggle_engine(&order, SttEngine::Local);
        assert_eq!(order, vec![SttEngine::Remote, SttEngine::Local]);
    }

    #[test]
    fn toggle_engine_removes_when_enabled_keeps_order() {
        let order = vec![SttEngine::Remote, SttEngine::Local, SttEngine::Native];
        let next = toggle_engine(&order, SttEngine::Local);
        assert_eq!(next, vec![SttEngine::Remote, SttEngine::Native]);
    }

    #[test]
    fn toggle_engine_refuses_to_drop_last_engine() {
        let order = vec![SttEngine::Remote];
        // Can't disable the only one — order must stay identical.
        assert_eq!(
            toggle_engine(&order, SttEngine::Remote),
            vec![SttEngine::Remote]
        );
    }

    #[test]
    fn toggle_engine_dedupes_after_remove() {
        // If the cascade somehow had a duplicate, removing one should still
        // leave a clean list. (Defensive — shouldn't happen in normal flow.)
        let order = vec![SttEngine::Remote, SttEngine::Local, SttEngine::Local];
        let next = toggle_engine(&order, SttEngine::Local);
        assert_eq!(next, vec![SttEngine::Remote]);
    }

    #[test]
    fn toggle_engine_round_trip_preserves_set() {
        // Disabling then re-enabling an engine should bring us back to the
        // original set, modulo the position (re-enabled goes to the end).
        let order = vec![SttEngine::Remote, SttEngine::Local, SttEngine::Native];
        let mid = toggle_engine(&order, SttEngine::Local);
        let end = toggle_engine(&mid, SttEngine::Local);
        assert_eq!(
            end,
            vec![SttEngine::Remote, SttEngine::Native, SttEngine::Local]
        );
    }

    #[test]
    fn legacy_settings_json_migrates_to_default_cascade() {
        // A settings.json written before `engine_order` existed should still
        // load — serde should apply `default_engine_order` for the missing field.
        let legacy = r#"{
            "base_url": "https://api.openai.com/v1",
            "api_key": null,
            "model": "whisper-1",
            "language": "ru",
            "realtime_preview": false,
            "recording_delay": 0.2,
            "hide_delay": 0.8,
            "hotkey": "fn",
            "activation_mode": "hold",
            "overlay_centered": false,
            "wake_server_on_start": true,
            "realtime_chunk_interval": 5.0,
            "local_fallback": false,
            "autostart": false
        }"#;
        let cfg: AppConfig = serde_json::from_str(legacy).expect("legacy json should parse");
        assert_eq!(cfg.engine_order, default_engine_order());
        assert_eq!(cfg.local_model, DEFAULT_LOCAL_MODEL);
        assert_eq!(cfg.remote_provider, DEFAULT_REMOTE_PROVIDER);
    }
}
