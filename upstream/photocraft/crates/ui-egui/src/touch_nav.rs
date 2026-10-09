//! Two-or-more-finger canvas navigation.
//!
//! A second finger on the canvas cancels any in-progress tool and keeps only pan and pinch-zoom
//! until every finger is lifted. One finger still drives the current tool. Pinch zoom itself is
//! applied by [`crate::wheel_nav`].

use egui::{Context, Vec2};

use crate::PhotocraftApp;

/// Navigation is active this frame if it already was, or a second finger just landed. The lift
/// frame must still count as navigation so a pointer-up cannot commit the cancelled tool.
pub fn active_this_frame(was_nav: bool, touches: usize) -> bool {
    was_nav || touches >= 2
}

/// Stay in navigation while any finger or pointer is still down.
pub fn next_latch(nav_this_frame: bool, touches: usize, pointer_down: bool) -> bool {
    touches >= 2 || (nav_this_frame && (touches > 0 || pointer_down))
}

/// How many fingers egui currently tracks.
pub fn touch_count(ctx: &Context) -> usize {
    ctx.input(|i| i.multi_touch().map_or(usize::from(i.any_touches()), |m| m.num_touches))
}

/// Update the latch, drop any in-progress tool, and return whether this frame is navigation-only.
pub fn update(app: &mut PhotocraftApp, ctx: &Context) -> bool {
    // A pen owns the canvas: leftover finger-nav must not abort the stroke.
    if app.stylus.is_pen() {
        app.touch_nav = false;
        return false;
    }
    let touches = touch_count(ctx);
    let pointer_down = ctx.input(|i| i.pointer.any_down());
    let one_finger_pan = app.session.prefs().tools.block_finger_input && app.stylus.is_finger() && (touches >= 1 || pointer_down);
    let nav = active_this_frame(app.touch_nav, touches) || one_finger_pan;
    if nav {
        abort_tools(app, ctx);
    }
    app.touch_nav = next_latch(nav, touches, pointer_down) || one_finger_pan;
    nav
}

/// Centroid pan of a two-finger gesture (points). `None` when only one finger remains.
pub fn pan_delta(ctx: &Context) -> Option<Vec2> {
    ctx.input(|i| i.multi_touch().map(|m| m.translation_delta)).filter(|d| d.x.is_finite() && d.y.is_finite() && *d != Vec2::ZERO)
}

/// Drop an in-progress tool without committing it.
pub fn abort_tools(app: &mut PhotocraftApp, ctx: &Context) {
    app.drag = None;
    app.live_stroke = None;
    app.trail = None;
    app.move_preview = None;
    app.alt_sampling = false;
    app.brush_resize = None;
    app.brush_resize_armed = false;
    app.secondary_erase = false;
    app.guide_drag = None;
    crate::move_mods::finish(app);
    crate::crop_ui::abort_drag(app);
    crate::gradient_ui::abort_drag(app);
    crate::transform_tool::abort_pointer(app);
    crate::zoom_tool::abort(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_finger_enters_navigation() {
        assert!(!active_this_frame(false, 0));
        assert!(!active_this_frame(false, 1));
        assert!(active_this_frame(false, 2));
        assert!(active_this_frame(false, 3));
        assert!(active_this_frame(true, 1));
        assert!(active_this_frame(true, 0), "the lift frame must still block the tool");
    }

    #[test]
    fn latch_holds_until_every_finger_is_up() {
        assert!(next_latch(true, 2, true));
        assert!(next_latch(true, 1, true), "the remaining finger still navigates");
        assert!(next_latch(true, 0, true), "pointer still down");
        assert!(!next_latch(true, 0, false));
        assert!(!next_latch(false, 1, true));
        assert!(next_latch(false, 2, true));
    }

    #[test]
    fn hostile_counts_never_enter_navigation_from_one_finger() {
        for n in [0, 1] {
            assert!(!active_this_frame(false, n));
            assert!(!next_latch(false, n, false));
        }
    }
}
