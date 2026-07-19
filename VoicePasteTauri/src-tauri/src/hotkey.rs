use crate::models::HotkeyKind;
use tauri::{AppHandle, Emitter};

/// Manages the global hotkey registration via Tauri's global-shortcut plugin.
pub struct HotkeyManager {
    registered_shortcut: Option<String>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            registered_shortcut: None,
        }
    }

    /// Register (or re-register) the global shortcut based on current settings.
    pub fn register(&mut self, app: &AppHandle, kind: HotkeyKind) -> Result<(), String> {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        // Unregister previous shortcut
        if let Some(ref prev) = self.registered_shortcut {
            let _ = app.global_shortcut().unregister(prev.as_str());
        }

        let shortcut_str = kind.shortcut_str();

        // Register the shortcut
        app.global_shortcut()
            .on_shortcut(shortcut_str, move |app: &AppHandle, _shortcut, event| {
                use tauri_plugin_global_shortcut::ShortcutState;
                match event.state {
                    ShortcutState::Pressed => {
                        let _ = app.emit("hotkey-pressed", ());
                    }
                    ShortcutState::Released => {
                        let _ = app.emit("hotkey-released", ());
                    }
                }
            })
            .map_err(|e| format!("Failed to register shortcut '{}': {}", shortcut_str, e))?;

        self.registered_shortcut = Some(shortcut_str.to_string());
        log::info!("Registered global shortcut: {}", shortcut_str);
        Ok(())
    }

    /// Unregister the current shortcut.
    pub fn unregister(&mut self, app: &AppHandle) {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        if let Some(ref shortcut) = self.registered_shortcut {
            let _ = app.global_shortcut().unregister(shortcut.as_str());
            self.registered_shortcut = None;
        }
    }
}
