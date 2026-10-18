//! Touch-accessible equivalents of Tab and Shift+Tab, kept outside the panels they hide.

use egui::{Align, Layout, Rect, UiBuilder, pos2};

use crate::{PhotocraftApp, icons};

pub(crate) fn button_size(pro: bool) -> f32 {
    if pro { 22.0 } else { 28.0 }
}

/// Reserve the right edge before laying out status text, so long messages cannot cover the buttons.
pub(crate) fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui, size: f32) -> Rect {
    let rect = ui.max_rect();
    let gap = 4.0;
    let mut buttons = ui.new_child(UiBuilder::new().id_salt("panel-controls").max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    buttons.spacing_mut().item_spacing.x = gap;
    // Host full screen (HarmonyOS only): the far-right button, set apart from the two panel
    // buttons by a wider gap and drawn as four corner arrows, unlike "Only show canvas".
    if crate::window_fullscreen::available(app) {
        let on = crate::window_fullscreen::active(app);
        let label = crate::window_fullscreen::label(on);
        let response = icons::button_with_icon_size(&mut buttons, crate::window_fullscreen::icon(on), size, 15.0, on, label);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, buttons.is_enabled(), label));
        if response.clicked() {
            crate::window_fullscreen::toggle(app);
        }
        buttons.add_space(8.0);
    }
    // Right-to-left layout places the canvas button immediately left of the sidebar button.
    for (icon, label, shortcut, command) in
        [("panel-right", tl!("Right sidebar"), "Shift+Tab", "window.toggle.dock"), ("canvas-only", tl!("Only show canvas"), "Tab", "window.togglePanels")]
    {
        let response = icons::button_with_icon_size(&mut buttons, icon, size, 14.0, false, &format!("{label} ({shortcut})"));
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, buttons.is_enabled(), label));
        if response.clicked() {
            let _ = crate::menus::invoke(app, buttons.ctx(), command, serde_json::json!({}));
        }
    }
    // Keep job cancellation accessible even when a long status message fills the bar.
    if app.session.has_jobs() {
        buttons.add_space(6.0);
        crate::jobs_ui::status_progress(app, &mut buttons);
    }
    let left = buttons.min_rect().left();
    Rect::from_min_max(rect.min, pos2((left - gap).max(rect.left()), rect.bottom()))
}

/// Match the adjacent zoom field's height while retaining Fit's text and horizontal padding.
pub(crate) fn fit_button(app: &mut PhotocraftApp, ui: &mut egui::Ui, zoom_rect: Rect) {
    let height = zoom_rect.height();
    let t = crate::theme::Tokens::get(ui.ctx());
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(14.0, 0.0);
            ui.spacing_mut().interact_size.y = height;
            let label = egui::RichText::new(tl!("Fit")).font(crate::theme::medium(13.0)).color(t.text);
            let width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(tl!("Fit").into(), crate::theme::medium(13.0), t.text).size().x) + 28.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            let rect = rect.translate(egui::vec2(0.0, zoom_rect.center().y - rect.center().y));
            ui.place(
                rect,
                egui::Button::new(label)
                    .min_size(egui::vec2(0.0, height))
                    .fill(t.field)
                    .stroke(egui::Stroke::new(1.0, t.field_border))
                    .corner_radius(t.radius_sm),
            )
        })
        .inner;
    if response.clicked()
        && let Some(view) = app.session.active_index().and_then(|i| app.ui.views.get_mut(i))
    {
        view.fit_pending = true;
    }
}

#[cfg(test)]
#[path = "status_panel_controls_tests.rs"]
mod tests;
