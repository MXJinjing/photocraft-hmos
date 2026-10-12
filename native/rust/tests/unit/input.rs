fn frame(ctx: &egui::Context, raw: egui::RawInput, ui: impl FnMut(&mut egui::Ui)) {
    let _ = ctx.run_logic(&raw, |_| {});
    let mut output = ctx.run_ui(raw, ui);
    output.textures_delta.clear();
}

#[test]
fn fingers_hide_the_rendered_brush_outline_but_pen_and_mouse_keep_it() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::Tool};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width": 256, "height": 256}))
        .unwrap();
    app.ui.tool = Tool::Brush;
    let render = |app: &mut PhotocraftApp, events: Vec<egui::Event>| {
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 800.),
            )),
            events,
            ..Default::default()
        };
        app.host_input(&ctx, &mut raw);
        let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
        let mut output = ctx.run_ui(raw, |ui| app.host_ui(ui));
        output.textures_delta.clear();
        output
    };
    for _ in 0..4 {
        render(&mut app, vec![]);
    }
    let p = app.last_canvas_rect.center();
    for (source, expected) in [
        (super::PointerSource::Mouse, true),
        (super::PointerSource::Touch, false),
        (super::PointerSource::Pen, true),
    ] {
        app.stylus.feed.report(source, None);
        let output = render(&mut app, vec![egui::Event::PointerMoved(p)]);
        let outline = output.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Circle(c) if c.center == p && c.stroke.width > 0.));
        assert_eq!(outline, expected, "source {source:?}");
    }
}

fn snapshot(state: &mut super::State, points: &[(u64, f32, f32)], events: &mut Vec<egui::Event>) {
    state.push(
        super::Input {
            kind: "touch".into(),
            points: points
                .iter()
                .map(|&(id, x, y)| super::Contact { id, x, y })
                .collect(),
            ..Default::default()
        },
        2.0,
        events,
    );
}

#[test]
fn native_two_finger_navigation_changes_the_view_without_editing_document() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::Tool};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width":256,"height":256}))
        .unwrap();
    app.ui.tool = Tool::Brush;
    let mut state = super::State {
        feed: app.stylus.feed.clone(),
        ..Default::default()
    };
    let render = |app: &mut PhotocraftApp, events: Vec<egui::Event>| {
        let mut raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 800.),
            )),
            ..Default::default()
        };
        app.host_input(&ctx, &mut raw);
        let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
        let mut output = ctx.run_ui(raw, |ui| app.host_ui(ui));
        output.textures_delta.clear();
    };
    for _ in 0..4 {
        render(&mut app, vec![]);
    }
    let p = app.last_canvas_rect.center();
    let revision = app.session.active().unwrap().revision;
    let mut events = vec![];
    snapshot(
        &mut state,
        &[
            (1, p.x * 2. - 100., p.y * 2.),
            (2, p.x * 2. + 100., p.y * 2.),
        ],
        &mut events,
    );
    render(&mut app, std::mem::take(&mut events));
    // egui initializes its gesture anchor from the preceding pointer frame.
    render(&mut app, vec![]);
    let old = app.ui.views[0].clone();
    snapshot(
        &mut state,
        &[
            (1, p.x * 2. - 120., p.y * 2. + 20.),
            (2, p.x * 2. + 160., p.y * 2. + 20.),
        ],
        &mut events,
    );
    render(&mut app, std::mem::take(&mut events));
    assert!(app.ui.views[0].zoom > old.zoom);
    assert_ne!(app.ui.views[0].center, old.center);
    snapshot(&mut state, &[], &mut events);
    render(&mut app, events);
    assert_eq!(
        app.session.active().unwrap().revision,
        revision,
        "navigation must not commit a brush stroke"
    );
}

#[test]
fn native_navigation_rejects_sidebar_and_preferences_origins() {
    use photocraft_ui_egui::{PhotocraftApp, Services};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width":256,"height":256}))
        .unwrap();
    let mut state = super::State {
        feed: app.stylus.feed.clone(),
        ..Default::default()
    };
    let render = |app: &mut PhotocraftApp, events: Vec<egui::Event>| {
        let mut raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 800.),
            )),
            ..Default::default()
        };
        app.host_input(&ctx, &mut raw);
        let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
        let mut output = ctx.run_ui(raw, |ui| app.host_ui(ui));
        output.textures_delta.clear();
    };
    for _ in 0..4 {
        render(&mut app, vec![]);
    }
    for preferences in [false, true] {
        if preferences {
            photocraft_ui_egui::prefs_ui::open_preferences(&mut app, "tools");
            for _ in 0..4 {
                render(&mut app, vec![]);
            }
        }
        let p = if preferences {
            let rects = ctx
                .data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new("pc-dialog-rects")))
                .unwrap();
            assert!(!rects.is_empty());
            rects[0].center()
        } else {
            egui::pos2(1180., 400.)
        };
        let old = app.ui.views[0].clone();
        let mut events = vec![];
        snapshot(
            &mut state,
            &[(1, p.x * 2. - 40., p.y * 2.), (2, p.x * 2. + 40., p.y * 2.)],
            &mut events,
        );
        render(&mut app, std::mem::take(&mut events));
        render(&mut app, vec![]);
        let canvas = app.last_canvas_rect.center();
        snapshot(
            &mut state,
            &[
                (1, canvas.x * 2. - 100., canvas.y * 2. + 20.),
                (2, canvas.x * 2. + 100., canvas.y * 2. + 20.),
            ],
            &mut events,
        );
        render(&mut app, std::mem::take(&mut events));
        assert_eq!(app.ui.views[0].zoom, old.zoom, "preferences={preferences}");
        assert_eq!(
            app.ui.views[0].center, old.center,
            "preferences={preferences}"
        );
        snapshot(&mut state, &[], &mut events);
        render(&mut app, events);
    }
}

#[test]
fn screen_contacts_produce_egui_pinch_and_centroid_pan_and_clean_release() {
    let ctx = egui::Context::default();
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(&mut state, &[(7, 100., 100.)], &mut events);
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |_| {},
    );
    snapshot(&mut state, &[(7, 100., 100.), (9, 300., 100.)], &mut events);
    let mut raw = egui::RawInput {
        events: std::mem::take(&mut events),
        ..Default::default()
    };
    frame(&ctx, std::mem::take(&mut raw), |ui| {
        assert_eq!(ui.input(|i| i.multi_touch().unwrap().num_touches), 2);
    });
    snapshot(&mut state, &[(7, 100., 120.), (9, 340., 120.)], &mut events);
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |ui| {
            let touch = ui.input(|i| i.multi_touch().unwrap());
            assert_eq!(touch.num_touches, 2);
            assert!((touch.zoom_delta - 1.2).abs() < 0.001);
            assert_eq!(touch.translation_delta, egui::vec2(10., 10.));
        },
    );
    snapshot(&mut state, &[(9, 340., 120.)], &mut events);
    assert!(
        state.pressed.is_none(),
        "do not transfer a tool drag to the remaining finger"
    );
    snapshot(&mut state, &[], &mut events);
    frame(
        &ctx,
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            assert!(!ui.input(|i| i.any_touches() || i.pointer.any_down()));
            assert!(ui.input(|i| i.pointer.hover_pos().is_none()));
        },
    );
    assert_eq!(state.feed.source(), super::PointerSource::Touch);
}

#[test]
fn pen_takeover_cancels_fingers_and_ignores_palms_until_lift() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(&mut state, &[(1, 10., 20.), (2, 30., 20.)], &mut events);
    state.push(
        super::Input {
            kind: "pointer".into(),
            source: super::PointerSource::Pen,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    assert!(state.touches.is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                egui::Event::Touch {
                    phase: egui::TouchPhase::Cancel,
                    ..
                }
            ))
            .count(),
        2
    );
    let before = events.len();
    snapshot(&mut state, &[(1, 20., 30.)], &mut events);
    assert_eq!(before, events.len());
    assert_eq!(state.feed.source(), super::PointerSource::Pen);
    state.push(
        super::Input {
            kind: "pointer".into(),
            source: super::PointerSource::Pen,
            action: 1,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    snapshot(&mut state, &[(1, 20., 30.)], &mut events);
    assert!(state.touches.is_empty());
    snapshot(&mut state, &[], &mut events);
    snapshot(&mut state, &[(3, 20., 30.)], &mut events);
    assert_eq!(state.touches.len(), 1);
    state.cancel(egui::Pos2::ZERO, &mut events);
    assert!(state.touches.is_empty() && state.pressed.is_none());
}

#[test]
fn pen_takeover_releases_finger_in_place_not_at_pen_tip() {
    let mut state = super::State::default();
    let mut events = vec![];
    // Finger primary at A (physical 10,20 → egui 5,10 with scale 2).
    snapshot(&mut state, &[(1, 10., 20.)], &mut events);
    assert_eq!(state.touch_pointer, Some(1));
    events.clear();
    // Pen down at B must not release the finger press at B (that rubber-bands A→B ink).
    push_pen(&mut state, 0, 400., 300., 0.5, &mut events);
    let finger = egui::pos2(5., 10.);
    let pen = egui::pos2(200., 150.);
    let release = events.iter().find_map(|e| match e {
        egui::Event::PointerButton {
            pressed: false,
            pos,
            ..
        } => Some(*pos),
        _ => None,
    });
    assert_eq!(
        release,
        Some(finger),
        "finger primary must release at finger A, not pen B: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerGone)),
        "finger pointer must PointerGone before pen owns the canvas: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            egui::Event::PointerButton {
                pressed: true,
                pos,
                ..
            } if *pos == pen
        )),
        "pen must still press at B: {events:?}"
    );
    assert!(state.touches.is_empty());
    assert!(state.ignore_fingers);
}

#[test]
fn pen_hover_while_finger_down_does_not_rubber_band_to_tip() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(&mut state, &[(1, 10., 20.)], &mut events);
    events.clear();
    // Pen hover must suppress the finger in place; a PointerMoved to the tip while the
    // finger button is still down is exactly the finger→tip straight line bug.
    push_pen(&mut state, 2, 400., 300., 0., &mut events);
    let finger = egui::pos2(5., 10.);
    assert!(
        events.iter().any(|e| matches!(
            e,
            egui::Event::PointerButton {
                pressed: false,
                pos,
                ..
            } if *pos == finger
        )),
        "hover takeover must release finger at A: {events:?}"
    );
    assert!(state.touches.is_empty());
    assert!(state.ignore_fingers);
    assert!(state.pressed.is_none(), "finger must not remain pressed across hover move");
    // Subsequent palm snapshots stay suppressed.
    let before = events.len();
    snapshot(&mut state, &[(1, 20., 30.)], &mut events);
    assert_eq!(before, events.len());
}

#[test]
fn two_finger_snapshot_does_not_synthesize_primary_tool_press() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(
        &mut state,
        &[(1, 100., 100.), (2, 300., 100.)],
        &mut events,
    );
    assert!(
        state.pressed.is_none() && state.touch_pointer.is_none(),
        "multi-touch must not own a tool primary press"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerMoved(_))),
        "multi-touch begin still needs a pointer hover anchor for egui pinch: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })),
        "no primary press on a two-finger begin: {events:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                egui::Event::Touch {
                    phase: egui::TouchPhase::Start,
                    ..
                }
            ))
            .count(),
        2
    );
}

#[test]
fn second_finger_cancels_one_finger_primary_in_place() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(&mut state, &[(1, 100., 100.)], &mut events);
    assert_eq!(state.touch_pointer, Some(1));
    events.clear();
    snapshot(
        &mut state,
        &[(1, 100., 100.), (2, 300., 100.)],
        &mut events,
    );
    let finger = egui::pos2(50., 50.);
    assert!(
        events.iter().any(|e| matches!(
            e,
            egui::Event::PointerButton {
                pressed: false,
                pos,
                ..
            } if *pos == finger
        )),
        "second finger must release the first primary at its anchor: {events:?}"
    );
    assert!(state.pressed.is_none() && state.touch_pointer.is_none());
    assert_eq!(state.touches.len(), 2);
}

#[test]
fn leftover_finger_after_pinch_does_not_synthesize_primary() {
    let mut state = super::State::default();
    let mut events = vec![];
    // Two-finger pinch / pan: navigation only.
    snapshot(
        &mut state,
        &[(1, 100., 100.), (2, 300., 100.)],
        &mut events,
    );
    assert!(state.touch_nav_active);
    assert!(state.pressed.is_none() && state.touch_pointer.is_none());
    events.clear();
    // One finger remains on screen after the other lifts — must not become Primary.
    snapshot(&mut state, &[(1, 120., 110.)], &mut events);
    assert!(
        state.pressed.is_none() && state.touch_pointer.is_none(),
        "leftover finger after multi-touch must not own a tool primary: pressed={:?} touch_pointer={:?}",
        state.pressed,
        state.touch_pointer
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })),
        "no primary press from leftover finger: {events:?}"
    );
    assert!(state.touch_nav_active, "nav rejection stays until all fingers lift");
    // Full lift clears nav latch so a later intentional single-finger can paint.
    snapshot(&mut state, &[], &mut events);
    assert!(!state.touch_nav_active);
    state.leftover_guard_until = None; // expire platform re-report window for a fresh tap
    events.clear();
    snapshot(&mut state, &[(3, 200., 200.)], &mut events);
    assert_eq!(state.touch_pointer, Some(3));
    assert_eq!(state.pressed, Some(egui::PointerButton::Primary));
}

#[test]
fn leftover_finger_after_empty_snapshot_does_not_synthesize_primary() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(
        &mut state,
        &[(1, 100., 100.), (2, 300., 100.)],
        &mut events,
    );
    events.clear();
    // Platform quirk: empty snapshot between multi-touch and the remnant contact.
    snapshot(&mut state, &[], &mut events);
    assert!(!state.touch_nav_active);
    assert!(state.leftover_guard_until.is_some());
    events.clear();
    // Remnant finger re-reported as a fresh single contact must not ink a dot.
    snapshot(&mut state, &[(1, 120., 110.)], &mut events);
    assert!(
        state.pressed.is_none() && state.touch_pointer.is_none(),
        "re-reported leftover after empty must not Primary: pressed={:?} touch_pointer={:?}",
        state.pressed,
        state.touch_pointer
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })),
        "no primary press after empty-gap leftover: {events:?}"
    );
    assert!(state.touch_nav_active);
    snapshot(&mut state, &[], &mut events);
    assert!(!state.touch_nav_active);
}

#[test]
fn trackpad_axis_scales_are_incremental_and_pan_has_an_anchor() {
    let mut state = super::State::default();
    let mut events = vec![];
    for (action, zoom) in [(1, 1.), (2, 1.2), (2, 1.5), (3, 1.5), (1, 1.), (2, 0.8)] {
        state.push(
            super::Input {
                kind: "axis".into(),
                action,
                zoom,
                x: 200.,
                y: 100.,
                ..Default::default()
            },
            2.,
            &mut events,
        );
    }
    let zooms: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let egui::Event::Zoom(z) = e {
                Some(*z)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(zooms, vec![1.2, 1.25, 0.8]);
    state.push(
        super::Input {
            kind: "axis".into(),
            x: 200.,
            y: 100.,
            dx: 40.,
            dy: -20.,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    assert!(
        matches!(events.last(), Some(egui::Event::MouseWheel { delta, .. }) if *delta == egui::vec2(20., -10.))
    );
    assert!(
        matches!(events.get(events.len()-2), Some(egui::Event::PointerMoved(p)) if *p == egui::pos2(100., 50.))
    );
    let ctx = egui::Context::default();
    frame(
        &ctx,
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            assert_eq!(
                ui.input(|i| i.pointer.hover_pos()),
                Some(egui::pos2(100., 50.))
            );
        },
    );
}

#[test]
fn pen_samples_finger_source_and_host_gestures_reach_the_shared_feed() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::Tool};
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    let mut input = super::State {
        feed: app.stylus.feed.clone(),
        ..Default::default()
    };
    let mut events = vec![];
    for (source, expected) in [
        ("pen", super::PointerSource::Pen),
        ("touch", super::PointerSource::Touch),
    ] {
        input.push(
            super::Input {
                kind: "pointer".into(),
                source: super::PointerSource::Pen,
                action: 1,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        let packet = format!(
            r#"{{"kind":"pointer","source":"{source}","action":0,"x":20,"y":40,"sample":{{"pressure":0.25,"tiltX":30}}}}"#
        );
        input.push(serde_json::from_str(&packet).unwrap(), 2., &mut events);
        assert_eq!(app.stylus.feed.source(), expected);
        if source == "pen" {
            assert_eq!(app.stylus.sample().unwrap().pressure, 0.25);
            assert_eq!(app.stylus.sample().unwrap().tilt_x, 30.);
        } else {
            assert!(app.stylus.is_finger());
            assert!(app.stylus.sample().is_none());
        }
    }
    let ctx = egui::Context::default();
    app.ui.tool = Tool::Brush;
    for (gesture, at) in [
        ("doubleTap", 1000),
        ("doubleTap", 2000),
        ("longPress", 3000),
    ] {
        input.push(serde_json::from_value(serde_json::json!({"kind":"stylus","packet":{"version":1,"type":"gesture","gesture":gesture,"timestampMs":at}})).unwrap(), 1., &mut events);
        photocraft_ui_egui::chrome_ui::drain_stylus_gestures(&mut app, &ctx);
        if at == 1000 {
            assert_eq!(app.ui.tool, Tool::Eraser);
        }
        if at == 2000 {
            assert_eq!(app.ui.tool, Tool::Brush);
        }
        if at == 3000 {
            assert!(app.ui.canvas_tool_menu.is_some() || app.ui.brush_picker.is_some());
        }
    }
    // Unknown protocol versions must not trigger actions.
    input.push(serde_json::from_str(r#"{"kind":"stylus","packet":{"version":2,"type":"gesture","gesture":"doubleTap","timestampMs":4000}}"#).unwrap(), 1., &mut events);
    assert!(app.stylus.feed.take_gestures().is_empty());
    input.cancel(egui::Pos2::ZERO, &mut events);
    assert!(app.stylus.feed.get().is_none());
}
use super::*;
#[test]
fn cancel_releases_pointer_and_scales_coordinates() {
    let mut state = State::default();
    let mut events = vec![];
    state.push(
        Input {
            kind: "pointer".into(),
            x: 200.,
            y: 100.,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    assert!(matches!(events.first(),Some(Event::PointerMoved(p)) if *p==Pos2::new(100.,50.)));
    state.cancel(Pos2::ZERO, &mut events);
    assert!(matches!(
        events.get(2),
        Some(Event::PointerButton { pressed: false, .. })
    ));
    assert!(state.pressed.is_none());
}
#[test]
fn pointer_and_wheel_preserve_held_modifiers_until_focus_loss() {
    let mut state = State::default();
    let mut events = vec![];
    state.push(
        Input {
            kind: "key".into(),
            ctrl: true,
            shift: true,
            ..Default::default()
        },
        1.,
        &mut events,
    );
    state.push(
        Input {
            kind: "pointer".into(),
            ..Default::default()
        },
        1.,
        &mut events,
    );
    assert!(
        matches!(events.last(), Some(Event::PointerButton { modifiers, .. }) if modifiers.ctrl && modifiers.shift)
    );
    state.push(
        Input {
            kind: "wheel".into(),
            y: 10.,
            ..Default::default()
        },
        1.,
        &mut events,
    );
    assert!(
        matches!(events.last(), Some(Event::MouseWheel { modifiers, .. }) if modifiers.ctrl && modifiers.shift)
    );
    state.push(
        Input {
            kind: "blur".into(),
            ..Default::default()
        },
        1.,
        &mut events,
    );
    assert_eq!(state.modifiers, Modifiers::NONE);
}
#[test]
fn shortcut_does_not_insert_text_and_bad_numbers_are_ignored() {
    let mut state = State::default();
    let mut events = vec![];
    state.push(
        Input {
            kind: "key".into(),
            code: 2035,
            text: "s".into(),
            ctrl: true,
            ..Default::default()
        },
        1.,
        &mut events,
    );
    assert_eq!(events.len(), 2);
    state.push(
        Input {
            kind: "pointer".into(),
            x: f32::NAN,
            ..Default::default()
        },
        1.,
        &mut events,
    );
    assert_eq!(events.len(), 2);
}

#[test]
fn pen_hover_move_sets_hover_pos_without_pressing() {
    let mut state = super::State::default();
    let mut events = vec![];
    let packet = r#"{"kind":"pointer","source":"pen","action":2,"x":200,"y":100,"sample":{"pressure":0,"tiltX":0,"tiltY":0,"eraser":false}}"#;
    state.push(serde_json::from_str(packet).unwrap(), 2., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(100., 50.)),
        "hover must be PointerMoved only: {events:?}"
    );
    assert!(state.pressed.is_none());
    assert!(!state.pen_down);
    assert_eq!(state.feed.source(), super::PointerSource::Pen);
    assert_eq!(
        state.feed.get().map(|s| s.pressure),
        Some(0.),
        "hover reports zero pressure so ink does not start"
    );
    let ctx = egui::Context::default();
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |ui| {
            assert_eq!(
                ui.input(|i| i.pointer.hover_pos()),
                Some(egui::pos2(100., 50.))
            );
            assert!(!ui.input(|i| i.pointer.any_down()));
        },
    );
    state.push(
        super::Input {
            kind: "hover_leave".into(),
            x: 200.,
            y: 100.,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    frame(
        &ctx,
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            assert!(ui.input(|i| i.pointer.hover_pos().is_none()));
            assert!(!ui.input(|i| i.pointer.any_down()));
        },
    );
}

fn push_pen(
    state: &mut super::State,
    action: i32,
    x: f32,
    y: f32,
    pressure: f32,
    events: &mut Vec<egui::Event>,
) {
    let packet = format!(
        r#"{{"kind":"pointer","source":"pen","action":{action},"x":{x},"y":{y},"sample":{{"pressure":{pressure},"tiltX":0,"tiltY":0,"eraser":false}}}}"#
    );
    state.push(serde_json::from_str(&packet).unwrap(), 2., events);
}

fn push_mouse(state: &mut super::State, x: f32, y: f32, events: &mut Vec<egui::Event>) {
    state.push(
        super::Input {
            kind: "pointer".into(),
            action: 2,
            x,
            y,
            ..Default::default()
        },
        2.,
        events,
    );
}

fn proximity(state: &mut super::State, kind: &str, events: &mut Vec<egui::Event>) {
    state.push(
        super::Input {
            kind: kind.into(),
            ..Default::default()
        },
        2.,
        events,
    );
}

#[test]
fn hover_leave_hides_preview_until_next_pen_hover_without_entry_notification() {
    let mut state = super::State::default();
    let mut events = vec![];
    let ctx = egui::Context::default();
    push_pen(&mut state, 2, 200., 100., 0., &mut events);
    push_pen(&mut state, 2, 600., 300., 0., &mut events);
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |_| {},
    );
    // Neither native nor ArkTS leave coordinates may restore the old cursor.
    proximity(&mut state, "hover_leave", &mut events);
    proximity(&mut state, "hover_leave", &mut events);
    assert!(
        events
            .iter()
            .all(|event| matches!(event, egui::Event::PointerGone))
    );
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |ui| {
            assert!(ui.input(|i| i.pointer.hover_pos().is_none()));
            assert!(!ui.input(|i| i.pointer.any_down()));
        },
    );
    // Repeat/echo the old coordinates after leave without restoring the tip.
    push_pen(&mut state, 2, 600., 300., 0., &mut events);
    push_pen(&mut state, 2, 200., 100., 0., &mut events);
    assert!(events.is_empty());
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Default),
        egui::CursorIcon::None
    );
    // Motion resumes even if there is no new onHover(true).
    push_pen(&mut state, 2, 640., 340., 0., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(320., 170.))
    );
}

#[test]
fn mouse_motion_restores_preview_after_pen_hover_leave() {
    let mut state = super::State::default();
    let mut events = vec![];
    push_pen(&mut state, 2, 600., 300., 0., &mut events);
    proximity(&mut state, "hover_leave", &mut events);
    events.clear();
    push_mouse(&mut state, 800., 400., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(400., 200.))
    );
    assert_eq!(state.feed.source(), super::PointerSource::Mouse);
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Crosshair),
        egui::CursorIcon::Crosshair
    );
    events.clear();
    // Pen hover can take over again without a proximity entry notification.
    push_pen(&mut state, 2, 900., 450., 0., &mut events);
    assert!(events.is_empty());
    push_pen(&mut state, 2, 920., 470., 0., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(460., 235.))
    );
    assert_eq!(state.feed.source(), super::PointerSource::Pen);
}

#[test]
fn pen_lift_releases_contact_then_hides_preview_until_new_input() {
    let mut state = super::State::default();
    let mut events = vec![];
    let ctx = egui::Context::default();
    push_pen(&mut state, 0, 200., 100., 0.4, &mut events);
    push_pen(&mut state, 2, 600., 300., 0.5, &mut events);
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |_| {},
    );
    push_pen(&mut state, 1, 600., 300., 0., &mut events);
    assert!(matches!(
        events.as_slice(),
        [
            egui::Event::PointerMoved(_),
            egui::Event::PointerButton { pressed: false, .. },
            egui::Event::PointerGone
        ]
    ));
    assert!(!state.pen_down);
    assert!(state.pressed.is_none());
    frame(
        &ctx,
        egui::RawInput {
            events: std::mem::take(&mut events),
            ..Default::default()
        },
        |ui| {
            assert!(ui.input(|i| i.pointer.hover_pos().is_none()));
            assert!(!ui.input(|i| i.pointer.any_down()));
        },
    );
    // Lifting into hover need not produce another proximity entry notification.
    push_pen(&mut state, 2, 600., 300., 0., &mut events);
    assert!(events.is_empty());
    push_pen(&mut state, 2, 800., 400., 0., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(400., 200.))
    );
}

#[test]
fn pen_lift_stationary_and_cached_moves_do_not_restore_preview() {
    let mut state = super::State::default();
    let mut events = vec![];
    push_mouse(&mut state, 40., 80., &mut events);
    push_pen(&mut state, 2, 200., 100., 0., &mut events);
    push_pen(&mut state, 0, 200., 100., 0.5, &mut events);
    push_pen(&mut state, 2, 600., 300., 0.5, &mut events);
    push_pen(&mut state, 1, 600., 300., 0., &mut events);
    events.clear();
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Default),
        egui::CursorIcon::None
    );
    for _ in 0..20 {
        for (x, y) in [(600., 300.), (601., 301.), (200., 100.)] {
            push_pen(&mut state, 2, x, y, 0., &mut events);
            assert!(
                events.is_empty(),
                "stationary/replayed pen coordinates must stay hidden: {events:?}"
            );
        }
        push_mouse(&mut state, 40., 80., &mut events);
        assert!(
            events.is_empty(),
            "cached mouse coordinates must stay hidden: {events:?}"
        );
        state.push(
            super::Input {
                kind: "pointer".into(),
                action: 1,
                x: 40.,
                y: 80.,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        assert!(
            events.is_empty(),
            "an unmatched mouse release must not restore preview"
        );
    }
    // No proximity entry notification is required to resume actual motion.
    push_pen(&mut state, 2, 620., 320., 0., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(310., 160.))
    );
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Default),
        egui::CursorIcon::Default
    );
}

#[test]
fn first_post_lift_hover_is_an_anchor_and_mouse_can_resume_independently() {
    let mut state = super::State::default();
    let mut events = vec![];
    push_pen(&mut state, 0, 200., 100., 0.5, &mut events);
    push_pen(&mut state, 1, 600., 300., 0., &mut events);
    events.clear();
    // Hover coordinates can shift on transition even without horizontal motion.
    for _ in 0..20 {
        push_pen(&mut state, 2, 700., 400., 0., &mut events);
        push_pen(&mut state, 2, 701., 401., 0., &mut events);
        assert!(events.is_empty());
        assert_eq!(
            state.cursor_icon(egui::CursorIcon::Default),
            egui::CursorIcon::None
        );
    }
    push_mouse(&mut state, 900., 500., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(450., 250.))
    );
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Default),
        egui::CursorIcon::Default
    );
    events.clear();
    push_pen(&mut state, 2, 700., 400., 0., &mut events);
    assert!(
        events.is_empty(),
        "a replayed pen position must not replace the real mouse"
    );
    push_pen(&mut state, 2, 720., 420., 0., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(360., 210.))
    );
}

#[test]
fn pointer_cancel_keeps_pen_preview_hidden_but_focus_blur_restores_normal_cursor() {
    for source in [super::PointerSource::Pen, super::PointerSource::Mouse] {
        let mut state = super::State::default();
        let mut events = vec![];
        push_pen(&mut state, 0, 200., 100., 0.5, &mut events);
        push_pen(&mut state, 1, 600., 300., 0., &mut events);
        events.clear();
        state.push(
            super::Input {
                kind: "pointer".into(),
                action: 3,
                source,
                x: 600.,
                y: 300.,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerGone))
        );
        events.clear();
        for _ in 0..20 {
            push_pen(&mut state, 2, 600., 300., 0., &mut events);
            assert!(events.is_empty());
            assert_eq!(
                state.cursor_icon(egui::CursorIcon::Default),
                egui::CursorIcon::None
            );
        }
        proximity(&mut state, "blur", &mut events);
        assert_eq!(
            state.cursor_icon(egui::CursorIcon::Default),
            egui::CursorIcon::Default
        );
    }
}

#[test]
fn next_pen_down_restores_preview_and_contact_hover_leave_does_not_end_stroke() {
    let mut state = super::State::default();
    let mut events = vec![];
    push_pen(&mut state, 2, 200., 100., 0., &mut events);
    proximity(&mut state, "hover_leave", &mut events);
    events.clear();
    push_pen(&mut state, 0, 800., 400., 0.5, &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p), egui::Event::PointerButton { pressed: true, .. }] if *p == egui::pos2(400., 200.))
    );
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Crosshair),
        egui::CursorIcon::Crosshair
    );
    events.clear();
    proximity(&mut state, "hover_leave", &mut events);
    push_pen(&mut state, 2, 200., 100., 0., &mut events);
    assert!(
        events.is_empty(),
        "hover notifications during contact must not end/move a stroke"
    );
    assert!(state.pen_down);
    assert!(state.pressed.is_some());
    push_pen(&mut state, 2, 900., 450., 0.5, &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(450., 225.))
    );
}

#[test]
fn hover_leave_preserves_keyboard_modifiers_and_blur_hides_preview() {
    let mut state = super::State::default();
    let mut events = vec![];
    state.push(
        super::Input {
            kind: "key".into(),
            ctrl: true,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    push_pen(&mut state, 2, 200., 100., 0., &mut events);
    events.clear();
    proximity(&mut state, "hover_leave", &mut events);
    assert!(matches!(events.as_slice(), [egui::Event::PointerGone]));
    assert!(state.modifiers.ctrl);
    events.clear();
    proximity(&mut state, "blur", &mut events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerGone))
    );
    assert!(!state.modifiers.ctrl);
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Default),
        egui::CursorIcon::Default
    );
    events.clear();
    push_pen(&mut state, 2, 800., 400., 0., &mut events);
    assert!(
        matches!(events.as_slice(), [egui::Event::PointerMoved(p)] if *p == egui::pos2(400., 200.))
    );
}

#[test]
fn rendered_brush_outline_hides_on_lift_or_leave_and_returns_on_next_hover_or_mouse() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::Tool};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width": 256, "height": 256}))
        .unwrap();
    app.ui.tool = Tool::Brush;
    let render = |app: &mut PhotocraftApp, events: Vec<egui::Event>| {
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 800.),
            )),
            events,
            ..Default::default()
        };
        app.host_input(&ctx, &mut raw);
        let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
        let mut output = ctx.run_ui(raw, |ui| app.host_ui(ui));
        output.textures_delta.clear();
        output
    };
    let outline = |output: &egui::FullOutput, pos: egui::Pos2| {
        output.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Circle(c) if c.center == pos && c.stroke.width > 0.))
    };
    for _ in 0..4 {
        render(&mut app, vec![]);
    }
    let start = app.last_canvas_rect.center();
    let end = start + egui::vec2(40., 30.);
    let revision = app.session.active().unwrap().revision;
    let mut state = super::State {
        feed: app.stylus.feed.clone(),
        ..Default::default()
    };
    let mut events = vec![];
    push_pen(&mut state, 2, start.x * 2., start.y * 2., 0., &mut events);
    render(&mut app, std::mem::take(&mut events));
    push_pen(&mut state, 2, end.x * 2., end.y * 2., 0., &mut events);
    assert!(outline(&render(&mut app, std::mem::take(&mut events)), end));
    proximity(&mut state, "hover_leave", &mut events);
    let output = render(&mut app, std::mem::take(&mut events));
    assert!(
        !outline(&output, start) && !outline(&output, end),
        "leave must hide the brush tip"
    );
    assert!(ctx.input(|i| i.pointer.hover_pos().is_none()));
    // Stationary and cached reports, including a small jitter, stay hidden.
    for _ in 0..20 {
        for p in [end, start, end + egui::vec2(0.5, 0.5)] {
            push_pen(&mut state, 2, p.x * 2., p.y * 2., 0., &mut events);
            let output = render(&mut app, std::mem::take(&mut events));
            assert!(ctx.input(|i| i.pointer.hover_pos().is_none()));
            assert_eq!(
                state.cursor_icon(output.platform_output.cursor_icon),
                egui::CursorIcon::None
            );
        }
    }
    // Actual movement resumes without a separate hover-enter callback.
    let moved = end + egui::vec2(12., 8.);
    push_pen(&mut state, 2, moved.x * 2., moved.y * 2., 0., &mut events);
    assert!(outline(
        &render(&mut app, std::mem::take(&mut events)),
        moved
    ));
    // Once visible, regular hover tracks even a move back to the old position.
    push_pen(&mut state, 2, end.x * 2., end.y * 2., 0., &mut events);
    assert!(outline(&render(&mut app, std::mem::take(&mut events)), end));
    proximity(&mut state, "hover_leave", &mut events);
    render(&mut app, std::mem::take(&mut events));
    let mouse = start + egui::vec2(8., -10.);
    push_mouse(&mut state, mouse.x * 2., mouse.y * 2., &mut events);
    assert!(outline(
        &render(&mut app, std::mem::take(&mut events)),
        mouse
    ));
    assert!(!ctx.input(|i| i.pointer.any_down()));
    assert_eq!(
        state.cursor_icon(egui::CursorIcon::Default),
        egui::CursorIcon::Default
    );
    assert_eq!(app.session.active().unwrap().revision, revision);

    // Contact, lift and continued hover while the pen stays in proximity.
    push_pen(&mut state, 0, start.x * 2., start.y * 2., 0.5, &mut events);
    render(&mut app, std::mem::take(&mut events));
    push_pen(&mut state, 2, end.x * 2., end.y * 2., 0.5, &mut events);
    render(&mut app, std::mem::take(&mut events));
    push_pen(&mut state, 1, end.x * 2., end.y * 2., 0., &mut events);
    // Real callbacks may be batched into the very same frame as pen-up.
    push_pen(&mut state, 2, end.x * 2., end.y * 2., 0., &mut events);
    let output = render(&mut app, std::mem::take(&mut events));
    assert!(
        !outline(&output, start) && !outline(&output, end),
        "lift must hide the brush tip"
    );
    assert!(ctx.input(|i| i.pointer.hover_pos().is_none()));
    assert!(!ctx.input(|i| i.pointer.any_down()));
    assert_eq!(
        state.cursor_icon(output.platform_output.cursor_icon),
        egui::CursorIcon::None
    );
    let stroke_revision = app.session.active().unwrap().revision;
    assert!(
        stroke_revision > revision,
        "lift must still finish the stroke"
    );
    for _ in 0..20 {
        for p in [end, start, end + egui::vec2(0.5, 0.5)] {
            push_pen(&mut state, 2, p.x * 2., p.y * 2., 0., &mut events);
            let output = render(&mut app, std::mem::take(&mut events));
            assert!(!outline(&output, start) && !outline(&output, end));
            assert!(ctx.input(|i| i.pointer.hover_pos().is_none()));
            assert_eq!(
                state.cursor_icon(output.platform_output.cursor_icon),
                egui::CursorIcon::None
            );
        }
    }
    push_pen(&mut state, 2, moved.x * 2., moved.y * 2., 0., &mut events);
    assert!(outline(&render(&mut app, events), moved));
    assert!(!ctx.input(|i| i.pointer.any_down()));
    assert_eq!(
        app.session.active().unwrap().revision,
        stroke_revision,
        "hover must not paint"
    );
}

#[test]
fn stylus_connection_metadata_reaches_native_menu_and_clears_on_disconnect() {
    let mut input = super::State::default();
    let mut events = vec![];
    input.push(serde_json::from_str(r#"{"kind":"stylus","packet":{"version":1,"type":"connection","connected":true,"deviceName":"HUAWEI M-Pencil","batteryPercent":82}}"#).unwrap(), 1., &mut events);
    assert_eq!(input.feed.device().name.as_deref(), Some("HUAWEI M-Pencil"));
    assert_eq!(input.feed.device().battery_percent, Some(82));
    input.push(
        serde_json::from_str(
            r#"{"kind":"stylus","packet":{"version":1,"type":"connection","connected":false}}"#,
        )
        .unwrap(),
        1.,
        &mut events,
    );
    assert_eq!(input.feed.device(), Default::default());
    assert!(
        events.is_empty(),
        "status updates do not generate drawing input"
    );
}

#[test]
fn axis_does_not_reclassify_a_finger_as_mouse() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(&mut state, &[(1, 200., 200.)], &mut events);
    state.push(
        super::Input {
            kind: "axis".into(),
            action: 2,
            zoom: 1.1,
            x: 200.,
            y: 200.,
            ..Default::default()
        },
        2.,
        &mut events,
    );
    assert_eq!(state.feed.source(), super::PointerSource::Touch);
}

#[test]
fn mouse_echo_after_pinch_cannot_press_or_move_a_tool() {
    let mut state = super::State::default();
    let mut events = vec![];
    snapshot(&mut state, &[(1, 200., 200.), (2, 400., 200.)], &mut events);
    for empty in [false, true] {
        snapshot(
            &mut state,
            if empty { &[] } else { &[(1, 200., 200.)] },
            &mut events,
        );
        events.clear();
        for action in [0, 2, 1] {
            state.push(
                super::Input {
                    kind: "pointer".into(),
                    action,
                    x: 200.,
                    y: 200.,
                    ..Default::default()
                },
                2.,
                &mut events,
            );
        }
        assert!(events.is_empty(), "mouse echoes leaked: {events:?}");
        assert_eq!(state.feed.source(), super::PointerSource::Touch);
        assert!(state.pressed.is_none());
    }
    state.leftover_guard_until = None;
    state.push(
        super::Input {
            kind: "pointer".into(),
            ..Default::default()
        },
        2.,
        &mut events,
    );
    assert_eq!(state.pressed, Some(egui::PointerButton::Primary));
    assert_eq!(state.feed.source(), super::PointerSource::Mouse);
}

#[test]
fn blocked_finger_with_interleaved_axis_never_edits_the_document() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::Tool};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width":256,"height":256}))
        .unwrap();
    app.session
        .edit_prefs(|p| p.tools.block_finger_input = true);
    app.ui.tool = Tool::Brush;
    let mut state = super::State {
        feed: app.stylus.feed.clone(),
        ..Default::default()
    };
    let render = |app: &mut PhotocraftApp, events: Vec<egui::Event>| {
        let mut raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 800.),
            )),
            ..Default::default()
        };
        app.host_input(&ctx, &mut raw);
        let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
        let mut output = ctx.run_ui(raw, |ui| app.host_ui(ui));
        output.textures_delta.clear();
    };
    for _ in 0..4 {
        render(&mut app, vec![]);
    }
    let p = app.last_canvas_rect.center() * 2.;
    let revision = app.session.active().unwrap().revision;
    for _ in 0..10 {
        state.leftover_guard_until = None;
        let mut events = vec![];
        snapshot(&mut state, &[(1, p.x, p.y)], &mut events);
        // Last source in a batch must remain Touch even if axis arrives last.
        state.push(
            super::Input {
                kind: "axis".into(),
                action: 2,
                zoom: 1.01,
                x: p.x,
                y: p.y,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        render(&mut app, events);
        let mut events = vec![];
        snapshot(
            &mut state,
            &[(1, p.x - 30., p.y), (2, p.x + 30., p.y)],
            &mut events,
        );
        render(&mut app, events);
        let mut events = vec![];
        snapshot(
            &mut state,
            &[(1, p.x - 50., p.y), (2, p.x + 50., p.y)],
            &mut events,
        );
        render(&mut app, events);
        let mut events = vec![];
        snapshot(&mut state, &[], &mut events);
        state.push(
            super::Input {
                kind: "axis".into(),
                action: 3,
                zoom: 1.,
                x: p.x,
                y: p.y,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        for action in [0, 2, 1] {
            state.push(
                super::Input {
                    kind: "pointer".into(),
                    action,
                    x: p.x,
                    y: p.y,
                    ..Default::default()
                },
                2.,
                &mut events,
            );
        }
        render(&mut app, events);
        render(&mut app, vec![]);
        assert_eq!(app.session.active().unwrap().revision, revision);
    }
}

#[test]
fn staggered_lift_keeps_remaining_finger_owned_beyond_the_settling_window() {
    for remaining_id in [1, 2] {
        let mut state = super::State::default();
        let mut events = vec![];
        snapshot(&mut state, &[(1, 200., 200.), (2, 400., 200.)], &mut events);
        snapshot(&mut state, &[(remaining_id, 300., 200.)], &mut events);
        // A remaining contact outlives the timer: ownership comes from its lifetime.
        state.leftover_guard_until =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        events.clear();
        snapshot(&mut state, &[(remaining_id, 320., 210.)], &mut events);
        state.push(
            super::Input {
                kind: "axis".into(),
                action: 3,
                zoom: 1.2,
                x: 320.,
                y: 210.,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        for action in [0, 2, 1, 3] {
            state.push(
                super::Input {
                    kind: "pointer".into(),
                    action,
                    x: 320.,
                    y: 210.,
                    ..Default::default()
                },
                2.,
                &mut events,
            );
        }
        assert_eq!(state.feed.source(), super::PointerSource::Touch);
        assert_eq!(state.touches.len(), 1);
        assert!(state.touch_nav_active);
        assert!(state.pressed.is_none());
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerButton { pressed: true, .. }))
        );
        snapshot(&mut state, &[], &mut events);
        assert!(!state.touch_nav_active);
        assert!(state.touches.is_empty());
    }
}
