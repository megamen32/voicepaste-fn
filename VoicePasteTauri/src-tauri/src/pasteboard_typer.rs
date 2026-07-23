/// Cross-platform clipboard paste: copies text to clipboard and simulates Ctrl+V / Cmd+V.
pub struct PasteboardTyper;

impl PasteboardTyper {
    pub fn new() -> Self {
        Self
    }

    /// Copy text to clipboard and simulate paste keystroke.
    pub fn paste(&self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        // Copy to clipboard
        #[cfg(target_os = "macos")]
        {
            self.set_clipboard_macos(trimmed);
            self.simulate_paste_macos();
        }

        #[cfg(target_os = "windows")]
        {
            self.set_clipboard_windows(trimmed);
            self.simulate_paste_windows();
        }

        #[cfg(target_os = "linux")]
        {
            self.set_clipboard_linux(trimmed);
            self.simulate_paste_linux();
        }
    }

    // --- macOS ---
    #[cfg(target_os = "macos")]
    fn set_clipboard_macos(&self, text: &str) {
        use std::process::Command;
        let mut child = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .ok();
        if let Some(ref mut child) = child {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    #[cfg(target_os = "macos")]
    fn simulate_paste_macos(&self) {
        std::thread::sleep(std::time::Duration::from_millis(80));
        // Use AppleScript to simulate Cmd+V
        let _ = std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to keystroke \"v\" using command down",
            ])
            .output();
    }

    // --- Windows ---
    #[cfg(target_os = "windows")]
    fn set_clipboard_windows(&self, text: &str) {
        // Use PowerShell to set clipboard (cross-process safe)
        let _ = std::process::Command::new("powershell")
            .args([
                "-command",
                &format!("Set-Clipboard -Value '{}'", text.replace('\'', "''")),
            ])
            .output();
    }

    #[cfg(target_os = "windows")]
    fn simulate_paste_windows(&self) {
        std::thread::sleep(std::time::Duration::from_millis(80));
        // Simulate Ctrl+V via xdotool-equivalent on Windows
        // In Tauri, we can use the keyboard module or raw Win32
        // For now, use a simple approach
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            // Use PowerShell SendKeys as fallback
            let _ = Command::new("powershell")
                .args([
                    "-command",
                    "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('^v')",
                ])
                .output();
        }
    }

    // --- Linux ---
    #[cfg(target_os = "linux")]
    fn set_clipboard_linux(&self, text: &str) {
        // Try xclip first, then xsel
        let result = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()
            });

        if result.is_err() {
            let _ = std::process::Command::new("xsel")
                .args(["--clipboard", "--input"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(ref mut stdin) = child.stdin {
                        stdin.write_all(text.as_bytes())?;
                    }
                    child.wait()
                });
        }
    }

    #[cfg(target_os = "linux")]
    fn simulate_paste_linux(&self) {
        std::thread::sleep(std::time::Duration::from_millis(80));
        // Use xdotool to simulate Ctrl+V
        let _ = std::process::Command::new("xdotool")
            .args(["key", "ctrl+v"])
            .output();
    }
}
