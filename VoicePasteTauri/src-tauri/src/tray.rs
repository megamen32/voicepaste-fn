use crate::config::AppSettings;
use crate::models::{ActivationMode, HotkeyKind, Language};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Wry,
};

/// Builds and manages the system tray icon and context menu.
pub struct TrayManager {
    app: AppHandle,
}

impl TrayManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// Build the tray menu from current settings.
    pub fn build_menu(&self) -> Result<Menu<Wry>, String> {
        let settings = AppSettings::global().get();

        // Title
        let title = MenuItem::with_id(&self.app, "title", "VoicePaste", false, None::<&str>)
            .map_err(|e| e.to_string())?;

        // Settings submenu
        let endpoint = MenuItem::with_id(
            &self.app,
            "edit_endpoint",
            format!("Endpoint:  {}", settings.masked_base_url()),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let api_key = MenuItem::with_id(
            &self.app,
            "edit_api_key",
            format!("API Key:   {}", settings.masked_api_key()),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let settings_menu = Submenu::with_id_and_items(
            &self.app,
            "settings_submenu",
            "Settings",
            true,
            &[&endpoint as &dyn tauri::menu::IsMenuItem<Wry>, &api_key],
        )
        .map_err(|e| e.to_string())?;

        // Recording delay submenu
        let delay_choices = [0.2, 0.5, 1.0, 1.5, 2.0];
        let delay_items: Vec<MenuItem<Wry>> = delay_choices
            .iter()
            .map(|&d| {
                let checked = (settings.recording_delay - d).abs() < 0.01;
                MenuItem::with_id(
                    &self.app,
                    format!("rec_delay_{}", d),
                    format!("{}{}s", if checked { "✓ " } else { "   " }, d),
                    true,
                    None::<&str>,
                )
                .unwrap()
            })
            .collect();
        let delay_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = delay_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let rec_delay_menu = Submenu::with_id_and_items(
            &self.app,
            "rec_delay_submenu",
            format!("Recording delay: {}s", settings.recording_delay),
            true,
            &delay_refs,
        )
        .map_err(|e| e.to_string())?;

        // Preview hide delay submenu
        let hide_choices = [0.0, 0.5, 0.8, 1.0, 2.0, 3.0, 5.0];
        let hide_items: Vec<MenuItem<Wry>> = hide_choices
            .iter()
            .map(|&d| {
                let checked = (settings.hide_delay - d).abs() < 0.01;
                let text = if d == 0.0 {
                    format!("{}immediately", if checked { "✓ " } else { "   " })
                } else {
                    format!("{}{}s", if checked { "✓ " } else { "   " }, d)
                };
                MenuItem::with_id(
                    &self.app,
                    format!("hide_delay_{}", d),
                    text,
                    true,
                    None::<&str>,
                )
                .unwrap()
            })
            .collect();
        let hide_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = hide_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let hide_delay_menu = Submenu::with_id_and_items(
            &self.app,
            "hide_delay_submenu",
            format!("Preview hide delay: {}s", settings.hide_delay),
            true,
            &hide_refs,
        )
        .map_err(|e| e.to_string())?;

        // Language submenu
        let lang_items: Vec<MenuItem<Wry>> = Language::all_cases()
            .iter()
            .map(|lang| {
                let checked = settings.language == *lang;
                MenuItem::with_id(
                    &self.app,
                    format!("lang_{}", lang.whisper_lang()),
                    format!("{}{}", if checked { "✓ " } else { "   " }, lang.title()),
                    true,
                    None::<&str>,
                )
                .unwrap()
            })
            .collect();
        let lang_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = lang_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let lang_menu = Submenu::with_id_and_items(
            &self.app,
            "lang_submenu",
            format!("Language: {}", settings.language.whisper_lang()),
            true,
            &lang_refs,
        )
        .map_err(|e| e.to_string())?;

        // Model submenu
        let model_items: Vec<MenuItem<Wry>> = vec![
            MenuItem::with_id(
                &self.app,
                "model_auto",
                format!("{}auto", if settings.model == "auto" || settings.model == "whisper-1" { "✓ " } else { "   " }),
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?,
            MenuItem::with_id(
                &self.app,
                "model_refresh",
                "↻ Refresh models",
                true,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?,
        ];
        let model_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = model_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let model_menu = Submenu::with_id_and_items(
            &self.app,
            "model_submenu",
            format!("Model: {}", settings.model),
            true,
            &model_refs,
        )
        .map_err(|e| e.to_string())?;

        // Realtime submenu
        let realtime_toggle = MenuItem::with_id(
            &self.app,
            "toggle_realtime",
            format!("{}Realtime preview", if settings.realtime_preview { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let cadence_choices = [2.0, 5.0, 10.0, 15.0, 30.0];
        let cadence_items: Vec<MenuItem<Wry>> = cadence_choices
            .iter()
            .map(|&d| {
                let checked = (settings.realtime_chunk_interval - d).abs() < 0.01;
                MenuItem::with_id(
                    &self.app,
                    format!("realtime_cadence_{}", d),
                    format!("{}{}s", if checked { "✓ " } else { "   " }, d),
                    true,
                    None::<&str>,
                )
                .unwrap()
            })
            .collect();
        let cadence_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = cadence_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let cadence_menu = Submenu::with_id_and_items(
            &self.app,
            "realtime_cadence_submenu",
            format!("Cadence: {}s", settings.realtime_chunk_interval),
            true,
            &cadence_refs,
        )
        .map_err(|e| e.to_string())?;

        let realtime_menu = Submenu::with_id_and_items(
            &self.app,
            "realtime_submenu",
            "Realtime preview",
            true,
            &[&realtime_toggle as &dyn tauri::menu::IsMenuItem<Wry>, &cadence_menu],
        )
        .map_err(|e| e.to_string())?;

        // Hotkey submenu
        let hotkey_items: Vec<MenuItem<Wry>> = [
            HotkeyKind::RightAlt,
            HotkeyKind::F13,
            HotkeyKind::F14,
            HotkeyKind::F15,
            HotkeyKind::ScrollLock,
            HotkeyKind::CapsLock,
            HotkeyKind::Insert,
            HotkeyKind::RightShift,
            HotkeyKind::RightControl,
        ]
        .iter()
        .map(|kind| {
            let checked = settings.hotkey == *kind;
            MenuItem::with_id(
                &self.app,
                format!("hotkey_{}", kind.shortcut_str()),
                format!("{}{}", if checked { "✓ " } else { "   " }, kind.title()),
                true,
                None::<&str>,
            )
            .unwrap()
        })
        .collect();
        let hotkey_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = hotkey_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let hotkey_menu = Submenu::with_id_and_items(
            &self.app,
            "hotkey_submenu",
            format!("Hotkey: {}", settings.hotkey.title()),
            true,
            &hotkey_refs,
        )
        .map_err(|e| e.to_string())?;

        // Activation submenu
        let hold_checked = settings.activation_mode == ActivationMode::Hold;
        let toggle_checked = settings.activation_mode == ActivationMode::Toggle;
        let hold_item = MenuItem::with_id(
            &self.app,
            "activation_hold",
            format!("{}Hold (press to start, release to stop)", if hold_checked { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;
        let toggle_item = MenuItem::with_id(
            &self.app,
            "activation_toggle",
            format!("{}Toggle (press to start, press again to stop)", if toggle_checked { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;
        let activation_menu = Submenu::with_id_and_items(
            &self.app,
            "activation_submenu",
            format!("Activation: {}", if hold_checked { "Hold" } else { "Toggle" }),
            true,
            &[&hold_item as &dyn tauri::menu::IsMenuItem<Wry>, &toggle_item],
        )
        .map_err(|e| e.to_string())?;

        // Toggles
        let autostart = MenuItem::with_id(
            &self.app,
            "toggle_autostart",
            format!("{}Autostart", if settings.autostart { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let overlay_centered = MenuItem::with_id(
            &self.app,
            "toggle_overlay_centered",
            format!("{}Centre overlay on screen", if settings.overlay_centered { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let wake_server = MenuItem::with_id(
            &self.app,
            "toggle_wake_server",
            format!("{}Wake server on dictation start", if settings.wake_server_on_start { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let local_fallback = MenuItem::with_id(
            &self.app,
            "toggle_local_fallback",
            format!("{}Local fallback on server failure", if settings.local_fallback { "✓ " } else { "   " }),
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        // Permissions info — show actual status on macOS
        let perms = crate::check_permissions();
        let mic_status = if perms["microphone"].as_bool().unwrap_or(false) { "✓" } else { "✗" };
        let ax_status = if perms["accessibility"].as_bool().unwrap_or(false) { "✓" } else { "✗" };
        let perm_label = format!("ℹ Permissions: Mic {} / Accessibility {}", mic_status, ax_status);
        let permissions = MenuItem::with_id(
            &self.app,
            "permissions_info",
            &perm_label,
            true,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        // Quit
        let quit = MenuItem::with_id(&self.app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))
            .map_err(|e| e.to_string())?;

        // Build menu
        Menu::with_items(
            &self.app,
            &[
                &title as &dyn tauri::menu::IsMenuItem<Wry>,
                &PredefinedMenuItem::separator(&self.app).map_err(|e| e.to_string())?,
                &settings_menu,
                &rec_delay_menu,
                &hide_delay_menu,
                &lang_menu,
                &model_menu,
                &realtime_menu,
                &autostart,
                &hotkey_menu,
                &activation_menu,
                &overlay_centered,
                &wake_server,
                &local_fallback,
                &permissions,
                &PredefinedMenuItem::separator(&self.app).map_err(|e| e.to_string())?,
                &quit,
            ],
        )
        .map_err(|e| e.to_string())
    }

    /// Rebuild the tray menu (call after settings change).
    pub fn rebuild(&self) {
        if let Ok(menu) = self.build_menu() {
            if let Some(tray) = self.app.tray_by_id("main-tray") {
                let _ = tray.set_menu(Some(menu));
                let _ = tray.set_visible(true);
            }
        }
    }
}

/// Helper for Language to iterate all cases.
impl Language {
    pub fn all_cases() -> Vec<Language> {
        vec![Language::Ru, Language::En, Language::Auto]
    }
}
