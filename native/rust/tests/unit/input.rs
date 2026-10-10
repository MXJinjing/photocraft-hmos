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
