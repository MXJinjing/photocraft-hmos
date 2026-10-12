//! Two-or-more-finger canvas navigation.
//!
//! A second finger on the canvas cancels any in-progress tool and keeps only pan and pinch-zoom
//! until every finger is lifted. One finger still drives the current tool. Pinch zoom itself is
//! applied by [`crate::wheel_nav`] (egui's multi-touch zoom factor).
//!
//! A short grace after a lone Touch Start holds off tool presses so a staggered second finger can
//! enter navigation without a visible stroke flash. When grace ends with one finger still down,
//! [`take_deferred_press`] arms a one-shot press so painting still starts without a new
//! `primary_pressed` from the OS.
//!
//! HarmonyOS host extras (preserved from #23/#28): a pen owns the canvas so leftover finger-nav
//! cannot abort a stroke; Preferences › Tools › Block finger input still pans with one finger.

use egui::{Context, Event, Id, Pos2, TouchPhase, Vec2};

use crate::PhotocraftApp;

/// Frames to hold off a lone-finger tool after Touch Start (~50 ms at 60 Hz).
const PINCH_GRACE_FRAMES: u8 = 3;

#[derive(Clone, Copy, Default)]
struct Grace {
    /// Remaining suppress frames; 0 means inactive.
    left: u8,
    /// One-shot: grace just ended with a finger still down — canvas should start the tool.
    deferred_press: bool,
}

fn grace_id() -> Id {
    Id::new("pc-touch-pinch-grace")
}

fn with_grace(ctx: &Context, f: impl FnOnce(&mut Grace)) {
    ctx.data_mut(|d| {
        let mut g = d.get_temp::<Grace>(grace_id()).unwrap_or_default();
        f(&mut g);
        d.insert_temp(grace_id(), g);
    });
}

/// True while the lone-finger pinch grace is suppressing tool input (not multi-touch nav).
pub fn tools_suppressed(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Grace>(grace_id()).is_some_and(|g| g.left > 0))
}

/// Consume the one-shot deferred press armed when grace ends with a finger still down.
pub fn take_deferred_press(ctx: &Context) -> bool {
    let mut taken = false;
    with_grace(ctx, |g| {
        taken = g.deferred_press;
        g.deferred_press = false;
    });
    taken
}

/// Navigation is active this frame if it already was, or a second finger just landed. The lift
/// frame must still count as navigation so a pointer-up cannot commit the cancelled tool.
pub fn active_this_frame(was_nav: bool, touches: usize) -> bool {
    was_nav || touches >= 2
}

/// Stay in navigation while any finger is still down.
///
/// A leftover Primary after the last finger lifts must *not* keep the latch: that previously
/// delayed or swallowed a rapid second pinch/pan on touch screens.
pub fn next_latch(nav_this_frame: bool, touches: usize, pointer_down: bool) -> bool {
    let _ = pointer_down;
    touches >= 2 || (nav_this_frame && touches > 0)
}

/// How many fingers egui currently tracks.
pub fn touch_count(ctx: &Context) -> usize {
    ctx.input(|i| i.multi_touch().map_or(usize::from(i.any_touches()), |m| m.num_touches))
}

/// Origin eligibility belongs to a canvas, and remains fixed until all contacts are lifted.
#[derive(Clone, Default)]
struct Origins {
    contacts: Vec<(egui::TouchDeviceId, egui::TouchId)>,
    blocked: bool,
}

impl Origins {
    fn frame(&mut self, events: &[Event], free: impl Fn(Pos2) -> bool) -> bool {
        let mut eligible = !self.contacts.is_empty() && !self.blocked;
        for event in events {
            if let Event::Touch { device_id, id, phase, pos, .. } = event {
                let key = (*device_id, *id);
                match phase {
                    TouchPhase::Start => {
                        if self.contacts.is_empty() {
                            self.blocked = false;
                        }
                        if !self.contacts.contains(&key) {
                            self.contacts.push(key);
                            self.blocked |= !free(*pos);
                        }
                        eligible = !self.blocked;
                    }
                    TouchPhase::Move => {}
                    TouchPhase::End | TouchPhase::Cancel => self.contacts.retain(|k| *k != key),
                }
            }
        }
        eligible && !self.blocked
    }
}

/// Both fingers must start on this canvas's unobstructed surface. Moving in from a panel or
/// dialog cannot acquire it; a gesture that started here can continue outside the canvas.
pub fn eligible(ctx: &Context, response: &egui::Response, under_dialog: bool) -> bool {
    let id = response.id.with("touch-origins");
    let mut origins = ctx.data(|d| d.get_temp::<Origins>(id)).unwrap_or_default();
    let events = ctx.input(|i| i.events.clone());
    let eligible = origins.frame(&events, |p| {
        response.rect.contains(p)
            && if under_dialog {
                crate::dialogs::free_position(ctx, response.rect, p)
            } else {
                ctx.layer_id_at(p).is_none_or(|layer| layer == response.layer_id)
            }
    });
    ctx.data_mut(|d| d.insert_temp(id, origins));
    eligible
}

fn clear_grace(ctx: &Context) {
    ctx.data_mut(|d| {
        d.remove::<Grace>(grace_id());
    });
}

/// Update the latch / grace, drop in-progress tools when navigating or in grace, and return
/// whether this frame is multi-touch navigation-only (centroid pan / pinch).
///
/// Grace suppresses tools without taking the nav pan path, and arms [`take_deferred_press`] when
/// it ends with one finger still down so one-finger paint still works after the wait.
///
/// When Preferences › Tools › Block finger input is on, a lone finger pans (HMOS / stylus mode)
/// instead of painting — that path clears grace and takes nav.
pub fn update(app: &mut PhotocraftApp, ctx: &Context, eligible: bool) -> bool {
    // A pen owns the canvas: leftover finger-nav must not abort the stroke.
    if app.stylus.is_pen() || !eligible {
        app.touch_nav = false;
        clear_grace(ctx);
        return false;
    }
    let touches = touch_count(ctx);
    let pointer_down = ctx.input(|i| i.pointer.any_down());
    // HMOS / stylus: blocked finger pans with one contact (do not paint, do not grace-defer).
    let one_finger_pan =
        app.session.prefs().tools.block_finger_input && app.stylus.is_finger() && (touches >= 1 || pointer_down);

    if one_finger_pan {
        clear_grace(ctx);
        abort_tools(app, ctx);
        app.touch_nav = true;
        return true;
    }

    let lone_start = touches == 1
        && ctx.input(|i| i.events.iter().any(|e| matches!(e, Event::Touch { phase: TouchPhase::Start, .. })));

    if touches >= 2 {
        clear_grace(ctx);
    } else if touches == 0 && !pointer_down {
        // All contacts and the Primary are up — drop grace. If Primary is still down but the
        // harness/OS omitted a touch snapshot this frame, keep ticking so deferred press works.
        clear_grace(ctx);
    } else if lone_start {
        with_grace(ctx, |g| {
            if g.left == 0 && !g.deferred_press {
                g.left = PINCH_GRACE_FRAMES;
            }
        });
    }

    // Tick grace: suppress while left > 0 after the tick; on expiry with Primary down, defer press.
    with_grace(ctx, |g| {
        if g.left == 0 {
            return;
        }
        g.left = g.left.saturating_sub(1);
        if g.left == 0 {
            g.deferred_press = pointer_down && touches < 2;
        }
    });

    if tools_suppressed(ctx) {
        abort_tools(app, ctx);
    }

    let nav = active_this_frame(app.touch_nav, touches);
    if nav {
        abort_tools(app, ctx);
        clear_grace(ctx); // multi-touch owns the gesture; no deferred paint after
    }
    app.touch_nav = next_latch(nav, touches, pointer_down);
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

    fn touch(id: u64, phase: TouchPhase, x: f32) -> Event {
        Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(id), phase, pos: egui::pos2(x, 50.0), force: None }
    }

    #[test]
    fn outside_origins_cannot_acquire_canvas_after_moving_inside() {
        let mut origins = Origins::default();
        let free = |p: Pos2| p.x >= 100.0 && p.x <= 500.0;
        assert!(!origins.frame(&[touch(1, TouchPhase::Start, 50.0)], free));
        assert!(!origins.frame(&[touch(1, TouchPhase::Move, 200.0), touch(2, TouchPhase::Start, 300.0)], free));
        assert!(!origins.frame(&[touch(1, TouchPhase::End, 200.0)], free));
        assert!(!origins.frame(&[touch(3, TouchPhase::Start, 400.0)], free));
        assert!(!origins.frame(&[touch(2, TouchPhase::Cancel, 300.0), touch(3, TouchPhase::End, 400.0)], free));
        assert!(origins.frame(&[touch(1, TouchPhase::Start, 200.0), touch(2, TouchPhase::Start, 300.0)], free));
        assert!(origins.frame(&[touch(1, TouchPhase::Move, 10.0), touch(2, TouchPhase::Move, 600.0)], free));
        assert!(origins.frame(&[touch(1, TouchPhase::End, 10.0), touch(2, TouchPhase::End, 600.0)], free));
        assert!(!origins.frame(&[], free));
    }

    #[test]
    fn second_finger_on_overlay_rejects_even_when_centroid_is_on_canvas() {
        let mut origins = Origins::default();
        let free = |p: Pos2| p.x < 300.0;
        assert!(origins.frame(&[touch(1, TouchPhase::Start, 100.0)], free));
        assert!(!origins.frame(&[touch(2, TouchPhase::Start, 350.0)], free));
        assert!(!origins.frame(&[touch(2, TouchPhase::Move, 150.0)], free));
    }

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
    fn latch_holds_while_fingers_remain_not_on_leftover_primary() {
        assert!(next_latch(true, 2, true));
        assert!(next_latch(true, 1, true), "the remaining finger still navigates");
        assert!(
            !next_latch(true, 0, true),
            "leftover Primary alone must not keep the latch (blocks consecutive pinch)"
        );
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
