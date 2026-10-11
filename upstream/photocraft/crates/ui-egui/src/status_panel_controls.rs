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
    // Right-to-left layout places the canvas button immediately left of the sidebar button.
    for (icon, label, shortcut, command) in
        [("panel-right", tl!("Right sidebar"), "Shift+Tab", "window.toggle.dock"), ("maximize-2", tl!("Only show canvas"), "Tab", "window.togglePanels")]
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

/// Keep the original Fit button in Studio themes; match its text and width in the shorter Pro bar.
pub(crate) fn fit_button(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let response = if !t.pro {
        crate::widgets::secondary_button(ui, tl!("Fit"), 0.0)
    } else {
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(14.0, 1.0);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.add(
                egui::Button::new(egui::RichText::new(tl!("Fit")).font(crate::theme::medium(13.0)).color(t.text))
                    .fill(t.field)
                    .stroke(egui::Stroke::new(1.0, t.field_border))
                    .corner_radius(t.radius_sm),
            )
        })
        .inner
    };
    if response.clicked()
        && let Some(view) = app.session.active_index().and_then(|i| app.ui.views.get_mut(i))
    {
        view.fit_pending = true;
    }
}

#[cfg(test)]
#[path = "status_panel_controls_tests.rs"]
mod tests;
