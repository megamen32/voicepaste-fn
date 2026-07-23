use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Wry};

/// Manages the overlay window — shows recording status, transcription preview, etc.
pub struct OverlayManager {
    app: AppHandle<Wry>,
}

impl OverlayManager {
    pub fn new(app: AppHandle<Wry>) -> Self {
        Self { app }
    }

    fn overlay_window(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window("overlay")
    }

    pub fn show_recording(&self) {
        self.set_overlay_size(64, 44);
        self.emit_overlay_state("recording", None);
        self.show();
    }

    pub fn show_waiting(&self) {
        self.set_overlay_size(64, 44);
        self.emit_overlay_state("waiting", None);
        self.show();
    }

    pub fn show_preview(&self, text: &str) {
        self.set_overlay_size(360, 100);
        self.emit_overlay_state("preview", Some(text.to_string()));
        self.show();
    }

    pub fn show_retry(&self, error: &str) {
        self.set_overlay_size(64, 44);
        self.emit_overlay_state("error", Some(error.to_string()));
        self.show();
    }

    pub fn show_paste_error(&self, error: &str) {
        self.set_overlay_size(360, 100);
        self.emit_overlay_state("paste-error", Some(error.to_string()));
        self.show();
    }

    pub fn hide(&self) {
        if let Some(window) = self.overlay_window() {
            let _ = window.hide();
            let _ = window.set_size(tauri::LogicalSize::new(64u32, 44u32));
        }
    }

    fn set_overlay_size(&self, width: u32, height: u32) {
        if let Some(window) = self.overlay_window() {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
        }
    }

    fn show(&self) {
        if let Some(window) = self.overlay_window() {
            let _ = window.show();
            let _ = window.set_always_on_top(true);
        }
    }

    fn emit_overlay_state(&self, state: &str, text: Option<String>) {
        let payload = serde_json::json!({
            "state": state,
            "text": text.unwrap_or_default(),
        });
        let _ = self.app.emit("overlay-state", payload);
    }

    /// Position the overlay near the cursor or centered on screen.
    pub fn position_near_cursor(&self, centered: bool) {
        if let Some(window) = self.overlay_window() {
            let size = window
                .outer_size()
                .unwrap_or(tauri::PhysicalSize::new(200u32, 44u32));
            let w = size.width as i32;
            let h = size.height as i32;

            // Get monitor info for bounds
            if let Some(monitor) = window.current_monitor().ok().flatten() {
                let mon_size = monitor.size();
                let mon_pos = monitor.position();
                let screen_w = mon_size.width as i32;
                let screen_h = mon_size.height as i32;
                let screen_x = mon_pos.x;
                let screen_y = mon_pos.y;

                let (wx, wy) = if centered {
                    (screen_x + (screen_w - w) / 2, screen_y + (screen_h - h) / 2)
                } else {
                    match get_cursor_position() {
                        Some((cx, cy)) => {
                            let mut wx = cx + 14;
                            let mut wy = cy - 52;
                            // Keep within monitor bounds
                            if wx + w > screen_x + screen_w {
                                wx = screen_x + screen_w - w - 8;
                            }
                            if wx < screen_x {
                                wx = screen_x + 8;
                            }
                            if wy < screen_y {
                                wy = cy + 20;
                            }
                            if wy + h > screen_y + screen_h {
                                wy = screen_y + screen_h - h - 8;
                            }
                            (wx, wy)
                        }
                        None => {
                            // Fallback: center on monitor
                            (screen_x + (screen_w - w) / 2, screen_y + (screen_h - h) / 2)
                        }
                    }
                };

                let _ = window.set_position(tauri::Position::Physical(
                    tauri::PhysicalPosition::new(wx, wy),
                ));
            }
        }
    }
}

/// Get current cursor position using native APIs (fast, no subprocess).
fn get_cursor_position() -> Option<(i32, i32)> {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::CGEvent;
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
        let event = CGEvent::new(source).ok()?;
        let loc = event.location();
        Some((loc.x as i32, loc.y as i32))
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let output = Command::new("powershell")
            .args([
                "-command",
                "[System.Windows.Forms.Cursor]::Position | ConvertTo-Json",
            ])
            .output()
            .ok()?;
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                let x = json["X"].as_i64()? as i32;
                let y = json["Y"].as_i64()? as i32;
                return Some((x, y));
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let output = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
            .ok()?;
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            let mut x = None;
            let mut y = None;
            for line in s.lines() {
                if let Some(val) = line.strip_prefix("X=") {
                    x = val.parse::<i32>().ok();
                } else if let Some(val) = line.strip_prefix("Y=") {
                    y = val.parse::<i32>().ok();
                }
            }
            if let (Some(x), Some(y)) = (x, y) {
                return Some((x, y));
            }
        }
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    None
}
