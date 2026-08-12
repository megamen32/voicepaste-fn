use crate::models::HotkeyKind;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

/// Manages the global hotkey registration.
/// On macOS, uses a Swift helper for modifier-only keys via CGEvent tap.
/// Uses Tauri's global-shortcut plugin for regular shortcuts (F13-F15).
pub struct HotkeyManager {
    registered_shortcut: Option<String>,
    #[cfg(target_os = "macos")]
    modifier_process: Arc<Mutex<Option<Child>>>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            registered_shortcut: None,
            #[cfg(target_os = "macos")]
            modifier_process: Arc::new(Mutex::new(None)),
        }
    }

    /// Register (or re-register) the global shortcut based on current settings.
    pub fn register(&mut self, app: &AppHandle, kind: HotkeyKind) -> Result<(), String> {
        // Unregister previous shortcut
        self.unregister(app);

        // On macOS, for modifier-only keys, use the Swift helper
        #[cfg(target_os = "macos")]
        if kind.needs_modifier_monitor() {
            return self.register_modifier_monitor(app, kind);
        }

        // For regular shortcuts (F13-F15), use Tauri's global-shortcut plugin
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let shortcut_str = kind.shortcut_str();

        app.global_shortcut()
            .on_shortcut(shortcut_str, move |app: &AppHandle, _shortcut, event| {
                use tauri_plugin_global_shortcut::ShortcutState;
                match event.state {
                    ShortcutState::Pressed => {
                        let app_for_event = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            log::info!("Hotkey pressed: {}", shortcut_str);
                            let _ = app_for_event.emit("hotkey-pressed", ());
                        });
                    }
                    ShortcutState::Released => {
                        let app_for_event = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            log::info!("Hotkey released: {}", shortcut_str);
                            let _ = app_for_event.emit("hotkey-released", ());
                        });
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

        #[cfg(target_os = "macos")]
        {
            let mut proc = self.modifier_process.lock().unwrap();
            if let Some(mut child) = proc.take() {
                let _ = child.kill();
                log::info!("Stopped modifier monitor process");
            }
        }
    }

    /// Register a modifier-only key using the Swift helper on macOS.
    #[cfg(target_os = "macos")]
    fn register_modifier_monitor(
        &mut self,
        app: &AppHandle,
        kind: HotkeyKind,
    ) -> Result<(), String> {
        let hotkey_str = kind.to_modifier_string();
        let helper_path = modifier_monitor_path()?;

        log::info!(
            "Starting modifier monitor for {} at {:?}",
            hotkey_str,
            helper_path
        );

        // Spawn the Swift helper process
        let mut child = Command::new(&helper_path)
            .arg(&hotkey_str)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start modifier monitor: {}", e))?;

        // Read stdout in a separate thread
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let app_clone = app.clone();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        let event_type = json["type"].as_str().unwrap_or("");
                        let key = json["key"].as_str().unwrap_or("");

                        match event_type {
                            "info" => {
                                log::info!(
                                    "[ModifierMonitor] {}",
                                    json["message"].as_str().unwrap_or("")
                                );
                            }
                            "error" => {
                                let message = json["message"]
                                    .as_str()
                                    .unwrap_or("Modifier monitor error")
                                    .to_string();
                                log::error!("[ModifierMonitor] {}", message);
                                let app_for_event = app_clone.clone();
                                let _ = app_clone.run_on_main_thread(move || {
                                    let _ = app_for_event.emit("hotkey-error", message);
                                });
                            }
                            "pressed" => {
                                log::info!("Modifier pressed: {}", key);
                                let app_for_event = app_clone.clone();
                                let _ = app_clone.run_on_main_thread(move || {
                                    let _ = app_for_event.emit("hotkey-pressed", ());
                                });
                            }
                            "released" => {
                                log::info!("Modifier released: {}", key);
                                let app_for_event = app_clone.clone();
                                let _ = app_clone.run_on_main_thread(move || {
                                    let _ = app_for_event.emit("hotkey-released", ());
                                });
                            }
                            "suppressed" => {
                                log::info!(
                                    "Modifier tap suppressed: {} (reason: {})",
                                    key,
                                    json["reason"].as_str().unwrap_or("")
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        // Store the child process
        let mut proc = self.modifier_process.lock().unwrap();
        *proc = Some(child);

        log::info!("Modifier monitor started for {}", hotkey_str);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn modifier_monitor_path() -> Result<std::path::PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|error| error.to_string())?;
    let app_dir = exe_path
        .parent()
        .ok_or_else(|| "Could not find VoicePaste executable directory".to_string())?;
    let bundled_path = app_dir.join("modifier_monitor");
    if bundled_path.exists() {
        return Ok(bundled_path);
    }
    Ok(std::path::PathBuf::from("modifier_monitor"))
}

/// Read TCC permissions from the bundled helper itself. It has the same
/// signed app identity that owns the event tap; using `swift -e` here would
/// inspect the command-line compiler process instead of VoicePaste.app.
#[cfg(target_os = "macos")]
pub fn macos_permissions() -> Value {
    modifier_monitor_path()
        .ok()
        .and_then(|path| Command::new(path).arg("--permissions").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice(&output.stdout).ok())
        .unwrap_or_else(|| {
            serde_json::json!({
                "microphone": false,
                "speech_recognition": false,
                "accessibility": false,
                "input_monitoring": false,
            })
        })
}

/// Ask macOS for permissions that have not yet been decided. Already denied
/// permissions remain denied; macOS then directs the user to System Settings.
#[cfg(target_os = "macos")]
pub fn request_macos_permissions(include_speech: bool) -> Value {
    if let Ok(path) = modifier_monitor_path() {
        let mut command = Command::new(path);
        command.arg("--request-permissions");
        if include_speech {
            command.arg("--include-speech");
        }
        if let Ok(output) = command.output() {
            if output.status.success() {
                if let Ok(permissions) = serde_json::from_slice(&output.stdout) {
                    return permissions;
                }
            }
        }
    }
    macos_permissions()
}

#[cfg(not(target_os = "macos"))]
pub fn request_macos_permissions(_include_speech: bool) -> Value {
    serde_json::json!({
        "microphone": true,
        "speech_recognition": true,
        "accessibility": true,
        "input_monitoring": true,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn macos_permissions() -> Value {
    request_macos_permissions(false)
}

impl HotkeyKind {
    /// Check if this hotkey needs the modifier monitor on macOS.
    #[cfg(target_os = "macos")]
    pub fn needs_modifier_monitor(&self) -> bool {
        matches!(
            self,
            HotkeyKind::Fn
                | HotkeyKind::RightOption
                | HotkeyKind::RightControl
                | HotkeyKind::RightCommand
                | HotkeyKind::RightShift
                | HotkeyKind::CapsLock
        )
    }

    /// Convert to the string format expected by the Swift helper.
    #[cfg(target_os = "macos")]
    pub fn to_modifier_string(&self) -> String {
        match self {
            HotkeyKind::Fn => "fn".to_string(),
            HotkeyKind::RightOption => "right_option".to_string(),
            HotkeyKind::RightControl => "right_control".to_string(),
            HotkeyKind::RightCommand => "right_command".to_string(),
            HotkeyKind::RightShift => "right_shift".to_string(),
            HotkeyKind::CapsLock => "caps_lock".to_string(),
            _ => "".to_string(),
        }
    }
}
