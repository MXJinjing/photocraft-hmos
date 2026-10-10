//! Synchronize the editor's theme with the platform window, without per-frame notifications.
use photocraft_ui_egui::theme::ThemeKind;

/// Convert host pixels into egui points while rejecting invalid geometry from the UI bridge.
pub fn titlebar_geometry(right_px: f32, height_px: f32, scale: f32) -> egui::Vec2 {
    if !scale.is_finite() || scale <= 0.0 {
        return egui::Vec2::ZERO;
    }
    egui::vec2((right_px / scale).max(0.0), (height_px / scale).max(0.0))
}

#[derive(Default)]
pub struct ChromeSync {
    sent: Option<ThemeKind>,
}

impl ChromeSync {
    pub fn invalidate(&mut self) {
        self.sent = None;
    }

    pub fn update(&mut self, theme: ThemeKind) -> Option<String> {
        if self.sent == Some(theme) {
            return None;
        }
        self.sent = Some(theme);
        Some(theme.chrome_css())
    }
}

#[cfg(test)]
#[path = "../tests/unit/chrome.rs"]
mod tests;
