//! Bottom system-gesture inset (HarmonyOS navigation indicator).
//!
//! The ArkTS page grows the XComponent under the navigation indicator with
//! `ignoreLayoutSafeArea` + `LayoutPolicy.matchParent` (API 20) and reports the window geometry in
//! **physical pixels** (`bottom_inset` input: `dy` indicator height, `dx` window height, `y` top of
//! the surface, `x` layout height). The worker compares them with the real surface height, so an
//! inset exists only while the surface truly reaches under the indicator. egui lays out in points,
//! so the host converts with the same `native scale * zoom` as the title-bar geometry and hands the
//! result to egui as `RawInput::safe_area_insets`. Panels, windows and modals then stay above the
//! strip, while the host paints the strip itself so the surface reaches the screen edge.
//!
//! Only the host layer is involved: the upstream editor is not modified.

use egui::{Color32, Rect, Ui, UiBuilder, epaint::MarginF32};
use photocraft_ui_egui::PhotocraftApp;

/// The inset never takes more than this share of the window height, so a bogus platform value
/// cannot squeeze the editor away.
const MAX_HEIGHT_SHARE: f32 = 0.25;

/// What ArkTS last reported about the navigation indicator, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Indicator {
    /// Height of the navigation-indicator avoid area; 0 when the window does not touch it.
    pub height: f32,
    /// Window height.
    pub window_height: f32,
    /// Top edge of the editor surface in window coordinates.
    pub surface_top: f32,
    /// Layout height from `onAreaChange` (includes the bottom extension once layout grows).
    pub layout_height: f32,
}

impl Indicator {
    /// Pixels of the indicator strip that a surface of `surface_height` actually covers.
    ///
    /// Uses the taller of the native surface and the ArkTS layout height: layout often updates
    /// one frame before the NativeWindow resize callback, and either one alone can under-report
    /// while the other has already grown under the indicator.
    pub fn overlap(&self, surface_height: f32) -> f32 {
        let tallest = surface_height.max(self.layout_height);
        let values = [self.height, self.window_height, self.surface_top, tallest];
        if values.iter().any(|v| !v.is_finite() || *v < 0.0)
            || self.height <= 0.0
            || self.window_height <= 0.0
            || tallest <= 0.0
        {
            return 0.0;
        }
        let indicator_top = self.window_height - self.height;
        let surface_bottom = self.surface_top + tallest;
        (surface_bottom - indicator_top).clamp(0.0, self.height)
    }
}

/// Convert the platform's pixel inset into egui points, rejecting invalid geometry.
pub fn bottom_points(bottom_px: f32, scale: f32, height_points: f32) -> f32 {
    if !bottom_px.is_finite() || !scale.is_finite() || scale <= 0.0 || !height_points.is_finite() {
        return 0.0;
    }
    (bottom_px / scale)
        .max(0.0)
        .min(height_points.max(0.0) * MAX_HEIGHT_SHARE)
}

pub fn insets(bottom_points: f32) -> egui::SafeAreaInsets {
    egui::SafeAreaInsets(MarginF32 {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: bottom_points,
    })
}

/// The strip continues the status bar when it is shown, and the canvas backdrop otherwise.
fn strip_color(app: &PhotocraftApp, ctx: &egui::Context) -> Color32 {
    let hidden = app.ui.view.hides_chrome();
    if hidden {
        return Color32::BLACK;
    }
    let tokens = photocraft_ui_egui::theme::Tokens::get(ctx);
    if app.ui.panels.status_bar {
        tokens.chrome
    } else {
        tokens.canvas
    }
}

/// Lay the editor out inside `ctx.content_rect()` and fill what the safe area removed.
pub fn host_ui(app: &mut PhotocraftApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let full = ctx.viewport_rect();
    let content = ctx.content_rect();
    if content == full {
        app.host_ui(ui);
        return;
    }
    let strip = Rect::from_min_max(egui::pos2(full.left(), content.bottom()), full.max);
    if strip.height() > 0.0 {
        // `layer_painter` clips to the content rect; the strip lies outside it on purpose.
        ctx.layer_painter(egui::LayerId::background())
            .with_clip_rect(full)
            .rect_filled(strip, 0.0, strip_color(app, &ctx));
    }
    ui.scope_builder(UiBuilder::new().max_rect(content), |ui| app.host_ui(ui));
}

#[cfg(test)]
#[path = "../tests/unit/safe_inset.rs"]
mod tests;
