//! Input-aware hints shared by icon-only controls.
use egui::{Context, Id, Response};

use crate::stylus::PointerSource;

#[derive(Clone, Copy)]
struct Input {
    source: PointerSource,
    enabled: bool,
}

pub(crate) fn sync(ctx: &Context, source: PointerSource, enabled: bool) {
    let input_id = Id::new("toolbar-tooltip-input");
    let touch_press_id = Id::new("touch-button-tooltip-press");
    let dismiss = ctx.data(|d| d.get_temp::<Input>(input_id)).is_some_and(|old| old.source != source)
        || !enabled
        || egui::Popup::is_any_open(ctx)
        || ctx.input(|i| i.pointer.any_pressed() || i.smooth_scroll_delta != egui::Vec2::ZERO);
    ctx.data_mut(|d| {
        d.insert_temp(input_id, Input { source, enabled });
        if dismiss {
            d.remove::<(Id, f64)>(Id::new("touch-button-tooltip"));
            d.remove::<Id>(touch_press_id);
        }
    });
}

/// Mouse hints keep the normal hover delay. Pen hover is immediate. A finger tap leaves a hint
/// anchored to the button for two seconds, even after the host sends PointerGone on lift.
/// `show` preserves tool-flyout suppression and the user's tooltip preference.
pub(crate) fn label(response: Response, text: &str, show: bool) -> Response {
    let ctx = &response.ctx;
    let input = ctx.data(|d| d.get_temp::<Input>(Id::new("toolbar-tooltip-input"))).unwrap_or(Input { source: PointerSource::Mouse, enabled: true });
    let enabled = input.enabled && ctx.global_style().interaction.tooltip_delay.is_finite();
    if !enabled || !show || text.is_empty() {
        return response;
    }
    if input.source == PointerSource::Touch {
        let touch_id = Id::new("touch-button-tooltip");
        let press_id = Id::new("touch-button-tooltip-press");
        let now = ctx.input(|i| i.time);
        let (pressed, released) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary).then(|| i.pointer.interact_pos()).flatten(),
                i.pointer.button_released(egui::PointerButton::Primary).then(|| i.pointer.interact_pos()).flatten(),
            )
        });
        // Track the raw tap so disabled icon-only controls still explain themselves. Checking the
        // layer prevents a button under an open popup from taking the tap.
        if pressed.is_some_and(|pos| response.rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id)) {
            ctx.data_mut(|d| d.insert_temp(press_id, response.id));
        }
        let tapped = released.is_some_and(|pos| {
            response.rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id) && ctx.data(|d| d.get_temp::<Id>(press_id)) == Some(response.id)
        });
        if ctx.data(|d| d.get_temp::<Id>(press_id)) == Some(response.id) && released.is_some() {
            ctx.data_mut(|d| d.remove::<Id>(press_id));
        }
        if tapped {
            ctx.data_mut(|d| d.insert_temp(touch_id, (response.id, now + 2.0)));
        }
        let active = ctx.data(|d| d.get_temp::<(Id, f64)>(touch_id));
        if let Some((id, until)) = active
            && id == response.id
            && now < until
            && !egui::Popup::is_any_open(ctx)
        {
            egui::Tooltip::for_widget(&response).show(|ui| {
                ui.label(text);
            });
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((until - now).max(0.0)));
        }
        response
    } else if input.source == PointerSource::Pen {
        if response.contains_pointer()
            && ctx.input(|i| i.pointer.hover_pos().is_some_and(|pos| response.rect.contains(pos)) && !i.pointer.any_down())
            && !egui::Popup::is_any_open(ctx)
        {
            egui::Tooltip::for_widget(&response).show(|ui| {
                ui.label(text);
            });
        }
        response
    } else {
        response.on_hover_text(text).on_disabled_hover_text(text)
    }
}
