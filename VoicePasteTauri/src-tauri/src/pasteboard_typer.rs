/// Cross-platform clipboard paste: copies text to the clipboard and simulates
/// Ctrl+V / Cmd+V in the currently focused application.
pub struct PasteboardTyper;

impl PasteboardTyper {
    pub fn new() -> Self {
        Self
    }

    /// Paste into the active application, returning an actionable error when
    /// the platform clipboard or keyboard injection is unavailable.
    pub fn paste(&self, text: &str) -> Result<(), String> {
        self.paste_to_pid(text, None)
    }

    /// Paste to the application that owned focus when recording started.
    /// Keeping the target PID prevents the recording overlay or a settings
    /// window from receiving the completed transcript.
    pub fn paste_to_pid(&self, text: &str, target_pid: Option<i32>) -> Result<(), String> {
        let Some(trimmed) = normalized_text(text) else {
            return Ok(());
        };

        #[cfg(target_os = "macos")]
        {
            self.set_clipboard_macos(&trimmed)?;
            self.simulate_paste_macos(target_pid)?;
        }

        #[cfg(target_os = "windows")]
        {
            self.set_clipboard_windows(&trimmed)?;
            self.simulate_paste_windows()?;
        }

        #[cfg(target_os = "linux")]
        {
            let _ = target_pid;
            self.set_clipboard_linux(&trimmed)?;
            self.simulate_paste_linux()?;
        }

        #[cfg(target_os = "windows")]
        let _ = target_pid;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn set_clipboard_macos(&self, text: &str) -> Result<(), String> {
        use std::io::Write;
        use std::process::Command;

        let mut child = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| format!("Failed to start pbcopy: {}", error))?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|error| format!("Failed to write clipboard: {}", error))?;
        }
        let status = child
            .wait()
            .map_err(|error| format!("Failed to wait for pbcopy: {}", error))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("pbcopy exited with {}", status))
        }
    }

    #[cfg(target_os = "macos")]
    fn simulate_paste_macos(&self, target_pid: Option<i32>) -> Result<(), String> {
        use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        std::thread::sleep(std::time::Duration::from_millis(80));

        // AppleScript/System Events can target the wrong application when
        // VoicePaste is launched by LaunchServices. Post directly through
        // CoreGraphics so Cmd+V goes to the currently focused application.
        for key_down in [true, false] {
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|_| "Failed to create macOS keyboard event source".to_string())?;
            let event = CGEvent::new_keyboard_event(source, 9, key_down)
                .map_err(|_| "Failed to create macOS Cmd+V event".to_string())?;
            event.set_flags(CGEventFlags::CGEventFlagCommand);
            if let Some(pid) = target_pid {
                event.post_to_pid(pid);
            } else {
                event.post(CGEventTapLocation::Session);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn set_clipboard_windows(&self, text: &str) -> Result<(), String> {
        let output = std::process::Command::new("powershell")
            .args([
                "-command",
                &format!("Set-Clipboard -Value '{}'", text.replace('\'', "''")),
            ])
            .output()
            .map_err(|error| format!("Failed to start PowerShell clipboard: {}", error))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "PowerShell clipboard failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    #[cfg(target_os = "windows")]
    fn simulate_paste_windows(&self) -> Result<(), String> {
        std::thread::sleep(std::time::Duration::from_millis(80));
        let output = std::process::Command::new("powershell")
            .args([
                "-command",
                "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('^v')",
            ])
            .output()
            .map_err(|error| format!("Failed to start PowerShell paste: {}", error))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "PowerShell paste failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn set_clipboard_linux(&self, text: &str) -> Result<(), String> {
        use std::io::Write;

        let result = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()
            });

        if result.map(|status| status.success()).unwrap_or(false) {
            return Ok(());
        }

        let fallback = std::process::Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()
            });
        if fallback.map(|status| status.success()).unwrap_or(false) {
            Ok(())
        } else {
            Err("Neither xclip nor xsel is available for the Linux clipboard".to_string())
        }
    }

    #[cfg(target_os = "linux")]
    fn simulate_paste_linux(&self) -> Result<(), String> {
        std::thread::sleep(std::time::Duration::from_millis(80));
        let output = std::process::Command::new("xdotool")
            .args(["key", "ctrl+v"])
            .output()
            .map_err(|error| format!("Failed to start xdotool: {}", error))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "xdotool paste failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

/// Return the PID of the application that currently owns keyboard focus.
/// This is captured before the non-focusable overlay is shown.
#[cfg(target_os = "macos")]
pub fn frontmost_process_id() -> Option<i32> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to unix id of first process whose frontmost is true",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .ok()
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_process_id() -> Option<i32> {
    None
}

/// Trim transcript text before it reaches the clipboard. Kept separate so
/// every platform can test the same empty-input contract without touching a
/// real clipboard or focused window.
pub fn normalized_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalized_text;

    #[test]
    fn whitespace_only_text_is_skipped() {
        assert_eq!(normalized_text(" \n\t "), None);
    }

    #[test]
    fn transcript_is_trimmed_once_before_paste() {
        assert_eq!(
            normalized_text("  hello world  "),
            Some("hello world".to_string())
        );
    }
}
