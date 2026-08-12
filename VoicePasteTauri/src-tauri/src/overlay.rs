use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Wry};

const INDICATOR_WIDTH: u32 = 58;
const INDICATOR_HEIGHT: u32 = 38;
const PREVIEW_MIN_WIDTH: u32 = 120;
const PREVIEW_MAX_WIDTH: u32 = 500;
const PREVIEW_SHORT_HEIGHT: u32 = 38;
const PREVIEW_MULTILINE_HEIGHT: u32 = 72;

/// Manages the overlay window — shows recording status, transcription preview, etc.
pub struct OverlayManager {
    app: AppHandle<Wry>,
}

impl OverlayManager {
    pub fn new(app: AppHandle<Wry>) -> Self {
        Self { app }
    }

    pub fn show_recording(&self, centered: bool) {
        log::info!(
            "overlay state: recording size={}x{}",
            INDICATOR_WIDTH,
            INDICATOR_HEIGHT
        );
        record_test_overlay_state("recording", INDICATOR_WIDTH, INDICATOR_HEIGHT);
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(window) = app.get_webview_window("overlay") else {
                log::error!("recording overlay window is not configured");
                return;
            };

            if let Err(error) =
                window.set_size(tauri::LogicalSize::new(INDICATOR_WIDTH, INDICATOR_HEIGHT))
            {
                log::error!("failed to size recording overlay: {}", error);
            }
            configure_macos_overlay(&window);
            if let Err(error) = window.set_always_on_top(true) {
                log::error!("failed to pin recording overlay: {}", error);
            }
            position_window_on_main(&window, centered);
            let _ = app.emit(
                "overlay-state",
                serde_json::json!({"state": "recording", "text": ""}),
            );
            if let Err(error) = window.show() {
                log::error!("failed to show recording overlay: {}", error);
            }
        });
    }

    pub fn show_waiting(&self) {
        log::info!(
            "overlay state: waiting size={}x{}",
            INDICATOR_WIDTH,
            INDICATOR_HEIGHT
        );
        record_test_overlay_state("waiting", INDICATOR_WIDTH, INDICATOR_HEIGHT);
        self.set_overlay_size(INDICATOR_WIDTH, INDICATOR_HEIGHT);
        self.emit_overlay_state("waiting", None);
        self.show();
    }

    pub fn show_preview(&self, text: &str) {
        let (width, height) = preview_size(text);
        log::info!(
            "overlay state: preview size={}x{} text_len={}",
            width,
            height,
            text.chars().count()
        );
        record_test_overlay_state("preview", width, height);
        self.set_overlay_size(width, height);
        self.emit_overlay_state("preview", Some(text.to_string()));
        self.show();
    }

    pub fn show_retry(&self, error: &str) {
        log::info!(
            "overlay state: retry size=360x118 error_len={}",
            error.len()
        );
        record_test_overlay_state("retry", 360, 118);
        // Retry errors contain actionable text and must not be squeezed into
        // the small recording dot.
        self.set_overlay_size(360, 118);
        self.emit_overlay_state("retry", Some(error.to_string()));
        self.show();
    }

    pub fn show_paste_error(&self, error: &str) {
        log::info!(
            "overlay state: paste-error size=320x92 error_len={}",
            error.len()
        );
        record_test_overlay_state("paste-error", 320, 92);
        self.set_overlay_size(320, 92);
        self.emit_overlay_state("paste-error", Some(error.to_string()));
        self.show();
    }

    pub fn hide(&self) {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(window) = app.get_webview_window("overlay") {
                if let Err(error) = window.hide() {
                    log::error!("failed to hide recording overlay: {}", error);
                }
                if let Err(error) =
                    window.set_size(tauri::LogicalSize::new(INDICATOR_WIDTH, INDICATOR_HEIGHT))
                {
                    log::error!("failed to reset recording overlay size: {}", error);
                }
            }
        });
    }

    fn set_overlay_size(&self, width: u32, height: u32) {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(window) = app.get_webview_window("overlay") {
                if let Err(error) = window.set_size(tauri::LogicalSize::new(width, height)) {
                    log::error!(
                        "failed to resize overlay to {}x{}: {}",
                        width,
                        height,
                        error
                    );
                }
            }
        });
    }

    fn show(&self) {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(window) = app.get_webview_window("overlay") {
                configure_macos_overlay(&window);
                if let Err(error) = window.set_always_on_top(true) {
                    log::error!("failed to pin overlay: {}", error);
                }
                if let Err(error) = window.show() {
                    log::error!("failed to show overlay: {}", error);
                }
            }
        });
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
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(window) = app.get_webview_window("overlay") else {
                return;
            };
            position_window_on_main(&window, centered);
        });
    }
}

/// Approximate AppKit's compact text sizing without growing a status update
/// into a fixed banner. The WebView applies the exact line wrapping; this only
/// reserves a sensible window width before it is shown.
fn preview_size(text: &str) -> (u32, u32) {
    let characters = text.chars().count() as u32;
    let width = characters
        .saturating_mul(7)
        .saturating_add(24)
        .clamp(PREVIEW_MIN_WIDTH, PREVIEW_MAX_WIDTH);
    let height = if characters > 60 {
        PREVIEW_MULTILINE_HEIGHT
    } else {
        PREVIEW_SHORT_HEIGHT
    };
    (width, height)
}

/// Optional lifecycle receipt for the real Computer Use canary. Production
/// launchers never set this path; when present, each state transition is
/// appended so the UI gate can distinguish lifecycle from clipboard-only work.
fn record_test_overlay_state(state: &str, width: u32, height: u32) {
    let Ok(path) = std::env::var("VOICEPASTE_TEST_OVERLAY_LOG") else {
        return;
    };
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{} {}x{}", state, width, height);
    }
}

fn position_window_on_main(window: &WebviewWindow, centered: bool) {
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(144u32, 112u32));
    let w = size.width as i32;
    let h = size.height as i32;
    let scale = window.scale_factor().unwrap_or(1.0);
    let cursor = get_cursor_position();

    // Use the monitor under the pointer when possible. `current_monitor()` can
    // still point at the old monitor while the transient overlay is hidden.
    let cursor_physical = cursor.map(|(x, y)| {
        (
            (x as f64 * scale).round() as i32,
            (y as f64 * scale).round() as i32,
        )
    });
    let monitor = window
        .available_monitors()
        .ok()
        .and_then(|monitors| {
            monitors.into_iter().find(|monitor| {
                let pos = monitor.position();
                let size = monitor.size();
                cursor_physical
                    .map(|(x, y)| {
                        x >= pos.x
                            && x < pos.x + size.width as i32
                            && y >= pos.y
                            && y < pos.y + size.height as i32
                    })
                    .unwrap_or(false)
            })
        })
        .or_else(|| window.current_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        let mon_size = monitor.size();
        let mon_pos = monitor.position();
        let (wx, wy) = calculate_overlay_position(
            cursor,
            (mon_pos.x, mon_pos.y),
            (mon_size.width as i32, mon_size.height as i32),
            (w, h),
            centered,
            if cfg!(target_os = "macos") {
                scale
            } else {
                1.0
            },
        );

        if let Err(error) = window.set_position(tauri::Position::Physical(
            tauri::PhysicalPosition::new(wx, wy),
        )) {
            log::error!("failed to position recording overlay: {}", error);
        }
    } else {
        log::warn!("could not find a monitor for recording overlay");
    }
}

/// Convert the cursor's logical coordinates to the physical coordinate space
/// used by Tauri's `PhysicalPosition`, then keep the overlay inside the active
/// monitor. macOS reports CGEvent cursor coordinates in points while a Retina
/// window/monitor is measured in pixels; mixing those spaces was the reason
/// the recording indicator could appear far away from the pointer.
pub(crate) fn calculate_overlay_position(
    cursor: Option<(i32, i32)>,
    monitor_origin: (i32, i32),
    monitor_size: (i32, i32),
    window_size: (i32, i32),
    centered: bool,
    cursor_scale: f64,
) -> (i32, i32) {
    let (screen_x, screen_y) = monitor_origin;
    let (screen_w, screen_h) = monitor_size;
    let (w, h) = window_size;
    let centered_position = || (screen_x + (screen_w - w) / 2, screen_y + (screen_h - h) / 2);

    if centered {
        return centered_position();
    }

    let Some((cx, cy)) = cursor else {
        return centered_position();
    };
    let cx = (cx as f64 * cursor_scale).round() as i32;
    let cy = (cy as f64 * cursor_scale).round() as i32;
    let mut wx = cx + 14;
    // Put the indicator below the pointer. If the pointer is near the bottom
    // edge, the function below flips it above the pointer instead.
    let mut wy = cy + 20;

    if wx + w > screen_x + screen_w {
        wx = screen_x + screen_w - w - 8;
    }
    if wx < screen_x {
        wx = screen_x + 8;
    }
    if wy + h > screen_y + screen_h {
        wy = cy - h - 12;
    }
    if wy < screen_y {
        wy = screen_y + 8;
    }
    (wx, wy)
}

/// Put the transient indicator into every Space, including a native
/// fullscreen Space. Tauri's always-on-top flag alone only applies to the
/// current Space on macOS.
#[cfg(target_os = "macos")]
fn configure_macos_overlay(window: &WebviewWindow) {
    let Ok(native_handle) = window.ns_window() else {
        return;
    };
    unsafe {
        let native_window: &objc2_app_kit::NSWindow = &*native_handle.cast();
        let behavior = native_window.collectionBehavior()
            | objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
            | objc2_app_kit::NSWindowCollectionBehavior::FullScreenAuxiliary
            | objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllApplications;
        native_window.setCollectionBehavior(behavior);
        native_window.setLevel(objc2_app_kit::NSStatusWindowLevel);
        native_window.setHidesOnDeactivate(false);
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_macos_overlay(_window: &WebviewWindow) {}

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

#[cfg(test)]
mod tests {
    use super::{calculate_overlay_position, preview_size};

    #[test]
    fn preview_stays_compact_for_short_transcript() {
        assert_eq!(preview_size("done"), (120, 38));
    }

    #[test]
    fn preview_grows_for_long_transcript_without_exceeding_cap() {
        assert_eq!(preview_size(&"a".repeat(61)), (451, 72));
        assert_eq!(preview_size(&"a".repeat(200)), (500, 72));
    }

    #[test]
    fn retina_cursor_coordinates_are_scaled_before_positioning() {
        let position = calculate_overlay_position(
            Some((500, 400)),
            (0, 0),
            (2880, 1800),
            (72, 56),
            false,
            2.0,
        );

        assert_eq!(position, (1014, 820));
    }

    #[test]
    fn overlay_is_centered_when_requested() {
        let position = calculate_overlay_position(
            Some((50, 50)),
            (100, 200),
            (1200, 800),
            (200, 44),
            true,
            2.0,
        );

        assert_eq!(position, (600, 578));
    }

    #[test]
    fn overlay_falls_back_to_center_without_cursor() {
        let position = calculate_overlay_position(None, (0, 0), (1000, 600), (200, 44), false, 1.0);

        assert_eq!(position, (400, 278));
    }

    #[test]
    fn overlay_flips_above_pointer_at_bottom_edge() {
        let position = calculate_overlay_position(
            Some((500, 880)),
            (0, 0),
            (2880, 1800),
            (72, 56),
            false,
            2.0,
        );

        assert_eq!(position, (1014, 1692));
    }
}
