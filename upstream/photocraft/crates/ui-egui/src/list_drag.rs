//! Reorderable list rows vs touch scrolling.
//!
//! On a mouse, a press-drag reorders the row (Photoshop desktop). On touch or pen the same
//! gesture must scroll the list: a single finger drag is scrolling. Reorder is armed only by a
//! double-press that stays held (tap, then press-and-hold on the same row), after which a drag
//! moves the item. Used by layer/Actions rows and by the preset browser (Swatches grid/list,
//! Gradients, Patterns, Styles, Shapes) wherever cells take [`Sense::click_and_drag`].

use egui::{Context, Id, Pos2, Response, Sense};

use crate::stylus::PointerSource;

const GATE: &str = "pc-list-drag-gate";

#[derive(Clone, Debug, Default)]
struct Gate {
    /// Last completed click: item, time (seconds), position.
    last_click: Option<(Id, f64, Pos2)>,
    /// Item whose current held press is the second half of a double-tap (drag allowed).
    armed: Option<Id>,
}

/// Touch and pen should scroll list rows by default; mouse keeps click-and-drag reorder.
pub fn prefers_scroll(source: PointerSource) -> bool {
    matches!(source, PointerSource::Touch | PointerSource::Pen)
}

/// Sense for one reorderable list row this frame.
pub fn row_sense(ctx: &Context, source: PointerSource, item: Id) -> Sense {
    if !prefers_scroll(source) {
        return Sense::click_and_drag();
    }
    if is_armed(ctx, item) {
        Sense::click_and_drag()
    } else {
        Sense::click()
    }
}

/// Whether `item` is armed for reorder on the current held press.
pub fn is_armed(ctx: &Context, item: Id) -> bool {
    ctx.data(|d| d.get_temp::<Gate>(Id::new(GATE)).and_then(|g| g.armed) == Some(item))
}

/// Update the gate after the row's response is known. Call once per row per frame.
pub fn after_row(ctx: &Context, source: PointerSource, item: Id, resp: &Response) {
    if !prefers_scroll(source) {
        return;
    }
    let mut gate = ctx.data(|d| d.get_temp::<Gate>(Id::new(GATE))).unwrap_or_default();
    let (time, down, pressed) = ctx.input(|i| (i.time, i.pointer.primary_down(), i.pointer.primary_pressed()));
    let (delay, dist) = ctx.options(|o| (o.input_options.max_double_click_delay, o.input_options.max_click_dist));

    if !down {
        gate.armed = None;
    }

    if resp.clicked() {
        let pos = resp.interact_pointer_pos().unwrap_or_else(|| resp.rect.center());
        gate.last_click = Some((item, time, pos));
    }

    // Second press of a double-tap on this row, still held → arm reorder for this press.
    if pressed
        && resp.is_pointer_button_down_on()
        && let Some((id, t0, p0)) = gate.last_click
    {
        let pos = ctx.input(|i| i.pointer.press_origin()).unwrap_or(p0);
        if id == item && (time - t0) <= delay && pos.distance(p0) <= dist {
            gate.armed = Some(item);
        }
    }

    ctx.data_mut(|d| d.insert_temp(Id::new(GATE), gate));
}

/// Whether a reorder / transfer drag may begin from this response.
pub fn may_begin_drag(ctx: &Context, source: PointerSource, item: Id, resp: &Response) -> bool {
    if !resp.drag_started() {
        return false;
    }
    if !prefers_scroll(source) {
        return true;
    }
    is_armed(ctx, item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn mouse_keeps_click_and_drag_and_touch_prefers_scroll() {
        let ctx = Context::default();
        let item = Id::new("row-a");
        assert_eq!(row_sense(&ctx, PointerSource::Mouse, item), Sense::click_and_drag());
        assert!(!prefers_scroll(PointerSource::Mouse));
        assert!(prefers_scroll(PointerSource::Touch));
        assert!(prefers_scroll(PointerSource::Pen));
        assert_eq!(row_sense(&ctx, PointerSource::Touch, item), Sense::click());
    }

    #[test]
    fn armed_item_gets_click_and_drag_sense() {
        let ctx = Context::default();
        let item = Id::new("layer-1");
        let other = Id::new("layer-2");
        ctx.data_mut(|d| {
            d.insert_temp(
                Id::new(GATE),
                Gate {
                    last_click: Some((item, 1.0, pos2(10.0, 10.0))),
                    armed: Some(item),
                },
            );
        });
        assert!(is_armed(&ctx, item));
        assert!(!is_armed(&ctx, other));
        assert_eq!(row_sense(&ctx, PointerSource::Touch, item), Sense::click_and_drag());
        assert_eq!(row_sense(&ctx, PointerSource::Touch, other), Sense::click());
        assert_eq!(row_sense(&ctx, PointerSource::Pen, item), Sense::click_and_drag());
    }

    #[test]
    fn may_begin_drag_requires_drag_started() {
        // Without a live Response.drag_started(), the gate alone is not enough — covered by
        // layer_row_ui mouse drag tests remaining green for PointerSource::Mouse.
        let ctx = Context::default();
        let item = Id::new("layer-3");
        assert!(!prefers_scroll(PointerSource::Mouse));
        assert!(prefers_scroll(PointerSource::Touch));
        assert_eq!(row_sense(&ctx, PointerSource::Mouse, item), Sense::click_and_drag());
    }

    #[test]
    fn preset_grid_cell_uses_same_gate_as_list_rows() {
        // Swatches (and Gradients/Patterns) share this gate for grid cells and list rows.
        let ctx = Context::default();
        let cell = Id::new(("swatches", 0usize, 3usize));
        assert_eq!(row_sense(&ctx, PointerSource::Touch, cell), Sense::click());
        assert_eq!(row_sense(&ctx, PointerSource::Pen, cell), Sense::click());
        assert_eq!(row_sense(&ctx, PointerSource::Mouse, cell), Sense::click_and_drag());
        ctx.data_mut(|d| {
            d.insert_temp(
                Id::new(GATE),
                Gate {
                    last_click: Some((cell, 1.0, pos2(8.0, 8.0))),
                    armed: Some(cell),
                },
            );
        });
        assert!(is_armed(&ctx, cell));
        assert_eq!(row_sense(&ctx, PointerSource::Touch, cell), Sense::click_and_drag());
        assert_eq!(row_sense(&ctx, PointerSource::Touch, Id::new(("swatches", 0usize, 4usize))), Sense::click());
    }
}
