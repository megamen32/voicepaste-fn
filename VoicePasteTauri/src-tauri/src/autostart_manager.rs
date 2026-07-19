/// Cross-platform autostart manager.
/// - macOS: LaunchAgent plist
/// - Windows: Registry HKCU\...\Run
/// - Linux: XDG autostart .desktop file

pub struct AutostartManager;

impl AutostartManager {
    /// Enable or disable autostart.
    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        if enabled {
            Self::enable()
        } else {
            Self::disable()
        }
    }

    /// Check if autostart is currently enabled.
    pub fn is_enabled() -> bool {
        #[cfg(target_os = "macos")]
        {
            Self::plist_path().exists()
        }
        #[cfg(target_os = "windows")]
        {
            Self::registry_key_exists()
        }
        #[cfg(target_os = "linux")]
        {
            Self::desktop_path().exists()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            false
        }
    }

    fn enable() -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            Self::enable_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::enable_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::enable_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("Autostart not supported on this platform".to_string())
        }
    }

    fn disable() -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            Self::disable_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::disable_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::disable_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err("Autostart not supported on this platform".to_string())
        }
    }

    // --- macOS ---
    #[cfg(target_os = "macos")]
    fn plist_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.bezrabotnyi.voicepaste.plist")
    }

    #[cfg(target_os = "macos")]
    fn enable_macos() -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot get exe path: {}", e))?;
        let plist_path = Self::plist_path();

        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create LaunchAgents dir: {}", e))?;
        }

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.bezrabotnyi.voicepaste</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
            exe.display()
        );

        std::fs::write(&plist_path, plist_content)
            .map_err(|e| format!("Cannot write plist: {}", e))
    }

    #[cfg(target_os = "macos")]
    fn disable_macos() -> Result<(), String> {
        let plist_path = Self::plist_path();
        if plist_path.exists() {
            std::fs::remove_file(&plist_path)
                .map_err(|e| format!("Cannot remove plist: {}", e))?;
        }
        Ok(())
    }

    // --- Windows ---
    #[cfg(target_os = "windows")]
    fn registry_key_exists() -> bool {
        use std::process::Command;
        let output = Command::new("reg")
            .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "VoicePaste"])
            .output();
        output.map(|o| o.status.success()).unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    fn enable_windows() -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot get exe path: {}", e))?;
        let exe_str = exe.to_string_lossy();

        let status = std::process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v", "VoicePaste",
                "/t", "REG_SZ",
                "/d", &exe_str,
                "/f",
            ])
            .status()
            .map_err(|e| format!("Cannot run reg add: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err("Failed to set registry key".to_string())
        }
    }

    #[cfg(target_os = "windows")]
    fn disable_windows() -> Result<(), String> {
        let _ = std::process::Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v", "VoicePaste",
                "/f",
            ])
            .status();
        Ok(())
    }

    // --- Linux ---
    #[cfg(target_os = "linux")]
    fn desktop_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(home)
            .join(".config")
            .join("autostart")
            .join("voicepaste.desktop")
    }

    #[cfg(target_os = "linux")]
    fn enable_linux() -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot get exe path: {}", e))?;
        let desktop_path = Self::desktop_path();

        if let Some(parent) = desktop_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create autostart dir: {}", e))?;
        }

        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=VoicePaste\n\
             Exec={}\n\
             X-GNOME-Autostart-enabled=true\n",
            exe.display()
        );

        std::fs::write(&desktop_path, content)
            .map_err(|e| format!("Cannot write desktop file: {}", e))
    }

    #[cfg(target_os = "linux")]
    fn disable_linux() -> Result<(), String> {
        let desktop_path = Self::desktop_path();
        if desktop_path.exists() {
            std::fs::remove_file(&desktop_path)
                .map_err(|e| format!("Cannot remove desktop file: {}", e))?;
        }
        Ok(())
    }
}
