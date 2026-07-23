use crate::config::AppSettings;
use crate::models::{ActivationMode, HotkeyKind, Language, SttEngine, UiLanguage, UiText};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Wry,
};

/// The tray is intentionally a small quick-controls surface. Provider keys,
/// model downloads, proxy details and permissions belong in the full window.
pub struct TrayManager {
    app: AppHandle,
}

impl TrayManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn build_menu(&self) -> Result<Menu<Wry>, String> {
        let config = AppSettings::global().get();
        let ui = config.effective_ui_language();

        let title = item(&self.app, "title", "VoicePaste", false)?;
        let record = item(
            &self.app,
            "open_record_window",
            &format!("🎙  {}", ui.text(UiText::RecordWindow)),
            true,
        )?;
        let activation = activation_menu(&self.app, &config.activation_mode, ui)?;
        let engines = engine_menu(&self.app, &config, ui)?;

        let mut top: Vec<Box<dyn tauri::menu::IsMenuItem<Wry>>> = Vec::new();
        top.push(Box::new(title));
        top.push(Box::new(separator(&self.app)?));
        top.push(Box::new(record));
        top.push(Box::new(activation));
        top.push(Box::new(engines));

        if config
            .engine_order
            .iter()
            .any(|engine| matches!(engine, SttEngine::Remote | SttEngine::Local))
        {
            top.push(Box::new(model_menu(&self.app, &config, ui)?));
        }

        top.push(Box::new(toggle_item(
            &self.app,
            "toggle_realtime",
            ui.text(UiText::RealtimePreview),
            config.realtime_preview,
        )?));
        top.push(Box::new(delay_menu(
            &self.app,
            "hide_delay_submenu",
            ui.text(UiText::PreviewHideDelay),
            "hide_delay_",
            config.hide_delay,
            &[0.0, 0.5, 0.8, 1.0, 2.0, 3.0, 5.0],
            ui,
        )?));
        top.push(Box::new(toggle_item(
            &self.app,
            "toggle_autostart",
            ui.text(UiText::Autostart),
            config.autostart,
        )?));
        top.push(Box::new(hotkey_menu(&self.app, &config.hotkey, ui)?));
        top.push(Box::new(delay_menu(
            &self.app,
            "rec_delay_submenu",
            ui.text(UiText::RecordingDelay),
            "rec_delay_",
            config.recording_delay,
            &[0.2, 0.5, 1.0, 1.5, 2.0],
            ui,
        )?));
        top.push(Box::new(language_menu(&self.app, config.language, ui)?));
        top.push(Box::new(toggle_item(
            &self.app,
            "toggle_overlay_centered",
            ui.text(UiText::CenterOverlay),
            config.overlay_centered,
        )?));
        top.push(Box::new(separator(&self.app)?));
        top.push(Box::new(item(
            &self.app,
            "open_settings",
            &format!("⚙  {}…", ui.text(UiText::Settings)),
            true,
        )?));
        top.push(Box::new(item(
            &self.app,
            "quit",
            ui.text(UiText::Quit),
            true,
        )?));

        let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
            top.iter().map(|entry| entry.as_ref()).collect();
        Menu::with_items(&self.app, &refs).map_err(|e| e.to_string())
    }

    pub fn rebuild(&self) {
        if let Ok(menu) = self.build_menu() {
            if let Some(tray) = self.app.tray_by_id("main-tray") {
                let _ = tray.set_menu(Some(menu));
                let _ = tray.set_visible(true);
            }
        }
    }
}

fn item(app: &AppHandle, id: &str, label: &str, enabled: bool) -> Result<MenuItem<Wry>, String> {
    MenuItem::with_id(app, id, label, enabled, None::<&str>).map_err(|e| e.to_string())
}

fn separator(app: &AppHandle) -> Result<PredefinedMenuItem<Wry>, String> {
    PredefinedMenuItem::separator(app).map_err(|e| e.to_string())
}

fn toggle_item(
    app: &AppHandle,
    id: &str,
    label: &str,
    checked: bool,
) -> Result<MenuItem<Wry>, String> {
    item(
        app,
        id,
        &format!("{}{}", if checked { "✓  " } else { "    " }, label),
        true,
    )
}

fn activation_menu(
    app: &AppHandle,
    current: &ActivationMode,
    ui: UiLanguage,
) -> Result<Submenu<Wry>, String> {
    let hold = item(
        app,
        "activation_hold",
        &format!(
            "{}{}",
            if *current == ActivationMode::Hold {
                "✓  "
            } else {
                "    "
            },
            ui.text(UiText::Hold)
        ),
        true,
    )?;
    let toggle = item(
        app,
        "activation_toggle",
        &format!(
            "{}{}",
            if *current == ActivationMode::Toggle {
                "✓  "
            } else {
                "    "
            },
            ui.text(UiText::Toggle)
        ),
        true,
    )?;
    Submenu::with_id_and_items(
        app,
        "activation_submenu",
        &format!(
            "{}: {}",
            ui.text(UiText::Activation),
            if *current == ActivationMode::Hold {
                ui.text(UiText::Hold)
            } else {
                ui.text(UiText::Toggle)
            }
        ),
        true,
        &[&hold as &dyn tauri::menu::IsMenuItem<Wry>, &toggle],
    )
    .map_err(|e| e.to_string())
}

fn engine_menu(
    app: &AppHandle,
    config: &crate::config::AppConfig,
    ui: UiLanguage,
) -> Result<Submenu<Wry>, String> {
    let local_available = if config.local_model == crate::local_transcriber::LOCAL_MODEL_PARAKEET_V3
    {
        config
            .local_command
            .as_deref()
            .map(|command| !command.trim().is_empty())
            .unwrap_or_else(|| std::env::var("PARAKEET_ASR_COMMAND").is_ok())
            && crate::local_transcriber::find_parakeet_model_dir().is_some()
    } else {
        crate::local_transcriber::LocalTranscriber::find_model_for(&config.local_model).is_some()
    };
    let native_available = crate::native_stt::NativeSttService::is_available();
    let entries: Vec<MenuItem<Wry>> = SttEngine::all_cases()
        .into_iter()
        .map(|engine| {
            let available = match engine {
                SttEngine::Remote => true,
                SttEngine::Local => local_available,
                SttEngine::Native => native_available,
            };
            let checked = config.engine_order.contains(&engine);
            let suffix = if available {
                "".to_string()
            } else if engine == SttEngine::Local {
                format!(" ({})", ui.text(UiText::NoModel))
            } else {
                format!(" ({})", ui.text(UiText::MacOnly))
            };
            item(
                app,
                engine.id(),
                &format!(
                    "{}{}{}",
                    if checked { "✓  " } else { "    " },
                    engine.title_for(ui),
                    suffix
                ),
                available,
            )
            .unwrap_or_else(|_| item(app, engine.id(), engine.title_for(ui), false).unwrap())
        })
        .collect();
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = entries
        .iter()
        .map(|entry| entry as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    Submenu::with_id_and_items(
        app,
        "engine_submenu",
        ui.text(UiText::SttEngine),
        true,
        &refs,
    )
    .map_err(|e| e.to_string())
}

fn model_menu(
    app: &AppHandle,
    config: &crate::config::AppConfig,
    ui: UiLanguage,
) -> Result<Submenu<Wry>, String> {
    let remote = item(
        app,
        "model_auto",
        &format!(
            "{}{}",
            if config.model == "auto" || config.model == "whisper-1" {
                "✓  "
            } else {
                "    "
            },
            format_model(ui, "remote_auto")
        ),
        true,
    )?;
    let whisper = item(
        app,
        "model_local_whisper",
        &format!(
            "{}{}",
            if config.local_model == crate::local_transcriber::LOCAL_MODEL_WHISPER_BASE {
                "✓  "
            } else {
                "    "
            },
            format_model(ui, "whisper")
        ),
        matches!(
            crate::local_transcriber::LocalTranscriber::model_status_for(
                crate::local_transcriber::LOCAL_MODEL_WHISPER_BASE
            ),
            crate::local_transcriber::ModelStatus::Present { .. }
        ),
    )?;
    let parakeet = item(
        app,
        "model_local_parakeet",
        &format!(
            "{}{}",
            if config.local_model == crate::local_transcriber::LOCAL_MODEL_PARAKEET_V3 {
                "✓  "
            } else {
                "    "
            },
            format_model(ui, "parakeet")
        ),
        matches!(
            crate::local_transcriber::LocalTranscriber::model_status_for(
                crate::local_transcriber::LOCAL_MODEL_PARAKEET_V3
            ),
            crate::local_transcriber::ModelStatus::Present { .. }
        ),
    )?;
    let settings = item(
        app,
        "open_settings",
        &format!("⚙  {}…", ui.text(UiText::Settings)),
        true,
    )?;
    let refs: [&dyn tauri::menu::IsMenuItem<Wry>; 4] = [&remote, &whisper, &parakeet, &settings];
    Submenu::with_id_and_items(
        app,
        "model_submenu",
        format!("{}: {}", ui.text(UiText::Model), model_summary(config)),
        true,
        &refs,
    )
    .map_err(|e| e.to_string())
}

fn format_model(ui: UiLanguage, model: &str) -> String {
    match (ui, model) {
        (UiLanguage::Ru, "remote_auto") => "Удалённая: Whisper API".to_string(),
        (UiLanguage::Zh, "remote_auto") => "远程：Whisper API".to_string(),
        (_, "remote_auto") => "Remote: Whisper API".to_string(),
        (UiLanguage::Ru, "whisper") => "Локальная: Whisper base".to_string(),
        (UiLanguage::Zh, "whisper") => "本地：Whisper base".to_string(),
        (_, "whisper") => "Local: Whisper base".to_string(),
        (UiLanguage::Ru, "parakeet") => "Локальная: Parakeet v3".to_string(),
        (UiLanguage::Zh, "parakeet") => "本地：Parakeet v3".to_string(),
        (_, "parakeet") => "Local: Parakeet v3".to_string(),
        _ => model.to_string(),
    }
}

fn model_summary(config: &crate::config::AppConfig) -> String {
    if config.engine_order.contains(&SttEngine::Remote) {
        config.model.clone()
    } else {
        config.local_model.clone()
    }
}

fn delay_menu(
    app: &AppHandle,
    id: &str,
    title: &str,
    prefix: &str,
    current: f64,
    choices: &[f64],
    ui: UiLanguage,
) -> Result<Submenu<Wry>, String> {
    let entries: Vec<MenuItem<Wry>> = choices
        .iter()
        .map(|choice| {
            let label = if *choice == 0.0 {
                ui.text(UiText::Immediately).to_string()
            } else {
                format!("{choice:.1}s")
            };
            item(
                app,
                &format!("{prefix}{choice}"),
                &format!(
                    "{}{}",
                    if (current - choice).abs() < 0.01 {
                        "✓  "
                    } else {
                        "    "
                    },
                    label
                ),
                true,
            )
            .unwrap()
        })
        .collect();
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = entries
        .iter()
        .map(|entry| entry as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    Submenu::with_id_and_items(app, id, &format!("{title}: {current:.1}s"), true, &refs)
        .map_err(|e| e.to_string())
}

fn hotkey_menu(
    app: &AppHandle,
    current: &HotkeyKind,
    ui: UiLanguage,
) -> Result<Submenu<Wry>, String> {
    let entries: Vec<MenuItem<Wry>> = HotkeyKind::all_cases()
        .into_iter()
        .map(|kind| {
            item(
                app,
                &format!("hotkey_{}", hotkey_id(kind)),
                &format!(
                    "{}{}",
                    if *current == kind { "✓  " } else { "    " },
                    kind.title_for(ui)
                ),
                true,
            )
            .unwrap()
        })
        .collect();
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = entries
        .iter()
        .map(|entry| entry as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    Submenu::with_id_and_items(
        app,
        "hotkey_submenu",
        &format!("{}: {}", ui.text(UiText::Hotkey), current.title_for(ui)),
        true,
        &refs,
    )
    .map_err(|e| e.to_string())
}

fn hotkey_id(kind: HotkeyKind) -> &'static str {
    match kind {
        HotkeyKind::Fn => "fn",
        HotkeyKind::RightOption => "right_option",
        HotkeyKind::RightControl => "right_control",
        HotkeyKind::RightCommand => "right_command",
        HotkeyKind::RightShift => "right_shift",
        HotkeyKind::CapsLock => "caps_lock",
        HotkeyKind::F13 => "f13",
        HotkeyKind::F14 => "f14",
        HotkeyKind::F15 => "f15",
    }
}

fn language_menu(
    app: &AppHandle,
    current: Language,
    ui: UiLanguage,
) -> Result<Submenu<Wry>, String> {
    let entries: Vec<MenuItem<Wry>> = Language::all_cases()
        .into_iter()
        .map(|language| {
            item(
                app,
                &format!("lang_{}", language.whisper_lang()),
                &format!(
                    "{}{}",
                    if current == language { "✓  " } else { "    " },
                    language.title_for(ui)
                ),
                true,
            )
            .unwrap()
        })
        .collect();
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = entries
        .iter()
        .map(|entry| entry as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    Submenu::with_id_and_items(
        app,
        "lang_submenu",
        &format!(
            "{}: {}",
            ui.text(UiText::SpeechLanguage),
            current.whisper_lang()
        ),
        true,
        &refs,
    )
    .map_err(|e| e.to_string())
}

impl Language {
    pub fn all_cases() -> Vec<Language> {
        vec![Language::Ru, Language::En, Language::Zh, Language::Auto]
    }
}
