//! Safe-area insets for the immersive HarmonyOS window (status bar on top, navigation indicator at
//! the bottom).
//!
//! The ArkTS page covers the whole window (`setWindowLayoutFullScreen`) and measures how much of the
//! top and bottom edge of the editor surface the system areas really cover; it sends that in
//! **physical pixels** (`safe_inset` input: `dx` top, `dy` bottom). Rust only converts units: it
//! divides by the same `native scale * zoom` as the title-bar geometry and hands the result to egui
//! as `RawInput::safe_area_insets`. Panels, windows and modals then stay inside the strips, while
//! the host paints the strips itself so the surface reaches the screen edges and no hole shows.
//!
//! Only the host layer is involved: the upstream editor is not modified.

use egui::{Color32, Rect, Ui, UiBuilder, epaint::MarginF32};
use photocraft_ui_egui::PhotocraftApp;

/// An inset never takes more than this share of the window height, so a bogus platform value
/// cannot squeeze the editor away.
const MAX_HEIGHT_SHARE: f32 = 0.25;

/// Top and bottom system-area heights last reported by ArkTS, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub struct Pixels {
    pub top: f32,
    pub bottom: f32,
}

/// Convert one platform inset into egui points, rejecting invalid geometry.
pub fn to_points(px: f32, scale: f32, height_points: f32) -> f32 {
    if !px.is_finite() || !scale.is_finite() || scale <= 0.0 || !height_points.is_finite() {
        return 0.0;
    }
    (px / scale)
        .max(0.0)
        .min(height_points.max(0.0) * MAX_HEIGHT_SHARE)
}

pub fn insets(top_points: f32, bottom_points: f32) -> egui::SafeAreaInsets {
    egui::SafeAreaInsets(MarginF32 {
        left: 0.0,
        right: 0.0,
        top: top_points,
        bottom: bottom_points,
    })
}

/// Colour of the strip above the content (continues the title bar) or below it.
/// Always opaque: even a bogus inset must never leave a transparent hole.
fn strip_color(app: &PhotocraftApp, ctx: &egui::Context, bottom: bool) -> Color32 {
    if app.ui.view.hides_chrome() {
        return Color32::BLACK;
    }
    let tokens = photocraft_ui_egui::theme::Tokens::get(ctx);
    let color = if bottom && !app.ui.panels.status_bar {
        tokens.canvas
    } else {
        tokens.chrome
    };
    color.to_opaque()
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
    // `layer_painter` clips to the content rect; the strips lie outside it on purpose.
    let painter = ctx
        .layer_painter(egui::LayerId::background())
        .with_clip_rect(full);
    if content.top() > full.top() {
        let strip = Rect::from_min_max(full.min, egui::pos2(full.right(), content.top()));
        painter.rect_filled(strip, 0.0, strip_color(app, &ctx, false));
    }
    if content.bottom() < full.bottom() {
        let strip = Rect::from_min_max(egui::pos2(full.left(), content.bottom()), full.max);
        painter.rect_filled(strip, 0.0, strip_color(app, &ctx, true));
    }
    ui.scope_builder(UiBuilder::new().max_rect(content), |ui| app.host_ui(ui));
}

#[cfg(test)]
#[path = "../tests/unit/safe_inset.rs"]
mod tests;
