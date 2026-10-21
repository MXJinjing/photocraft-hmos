use super::*;
use crate::stylus::{PenGesture, StylusFeed, StylusInput};
use serde_json::json;

#[test]
fn direction_dial_uses_screen_axes_and_has_no_bearing_when_upright() {
    assert_eq!(orientation(PenSample::default()), (0.0, None));
    for (x, y, bearing) in [(0.0, -45.0, 0.0), (45.0, 0.0, 90.0), (0.0, 45.0, 180.0), (-45.0, 0.0, 270.0)] {
        let (inclination, actual) = orientation(PenSample { tilt_x: x, tilt_y: y, ..Default::default() });
        assert!((inclination - 45.0).abs() < 0.01);
        assert!((actual.unwrap() - bearing).abs() < 0.01);
    }
    for x in [f32::NAN, f32::INFINITY, -90.0, 90.0] {
        let (inclination, bearing) = orientation(PenSample { tilt_x: x, tilt_y: 90.0, ..Default::default() });
        assert!(inclination.is_finite() && inclination <= 90.0);
        assert!(bearing.unwrap().is_finite());
    }
}

#[test]
fn display_rotation_puts_tilt_into_screen_axes() {
    let feed = StylusFeed::default();
    let up = Some(PenSample { pressure: 0.5, tilt_x: 0.0, tilt_y: -45.0, ..Default::default() });
    for (rotation, bearing) in [(0, 0.0), (1, 270.0), (2, 180.0), (3, 90.0)] {
        feed.set_display_rotation(rotation, false);
        feed.report(PointerSource::Pen, up);
        let d = feed.diagnostics();
        assert_eq!(d.display_rotation, rotation);
        let actual = orientation(d.last_pen.unwrap()).1.unwrap();
        assert!((actual - bearing).abs() < 0.01, "rotation {rotation}: {actual} != {bearing}");
    }
    feed.set_display_rotation(9, false);
    assert_eq!(feed.diagnostics().display_rotation, 3, "invalid quarter turns are ignored");
}

#[test]
fn landscape_adds_the_half_turn_reported_by_the_pad_pen_driver() {
    let feed = StylusFeed::default();
    let up = Some(PenSample { pressure: 0.5, tilt_x: 0.0, tilt_y: -45.0, ..Default::default() });
    for (rotation, bearing) in [(0, 180.0), (1, 90.0), (2, 0.0), (3, 270.0)] {
        feed.set_display_rotation(rotation, true);
        feed.report(PointerSource::Pen, up);
        let actual = orientation(feed.diagnostics().last_pen.unwrap()).1.unwrap();
        assert!((actual - bearing).abs() < 0.01, "landscape rotation {rotation}: {actual} != {bearing}");
    }
}

#[test]
fn diagnostics_retains_readings_without_stealing_painting_or_gestures() {
    let feed = StylusFeed::default();
    feed.report(PointerSource::Pen, Some(PenSample { pressure: 0.4, tilt_x: 30.0, tilt_y: -20.0, ..Default::default() }));
    feed.push_gesture(PenGesture::DoubleTap, 1000.0);
    feed.push_gesture(PenGesture::DoubleTap, 1100.0);
    feed.push_gesture(PenGesture::Squeeze, 2000.0);
    feed.push_gesture(PenGesture::Squeeze, f64::NAN);
    let a = feed.diagnostics();
    assert_eq!((a.samples, a.double_taps, a.squeezes), (1, 1, 1));
    assert_eq!(a.last_pen.unwrap().tilt_x, 30.0);
    assert_eq!(feed.drain().len(), 1, "reading diagnostics does not drain the stroke feed");
    assert_eq!(feed.take_gestures(), [PenGesture::DoubleTap, PenGesture::Squeeze]);
    feed.report(PointerSource::Mouse, None);
    assert_eq!(feed.diagnostics().last_pen, a.last_pen, "mouse echoes do not erase the last reading");
    assert_eq!(feed.get(), None, "normal painting still sees the actual current source");
}

#[test]
fn hover_has_its_own_state_and_point_without_erasing_contact_readings() {
    let feed = StylusFeed::default();
    let root = egui::ViewportId::ROOT;
    let child = egui::ViewportId::from_hash_of("hover-child");
    feed.report(PointerSource::Pen, Some(PenSample { pressure: 0.4, tilt_x: 30.0, tilt_y: -20.0, ..Default::default() }));
    feed.report_hover(root, [120.0, 340.0]);
    let hover = feed.diagnostics();
    assert!(hover.hovering);
    assert_eq!(hover.hover, Some([120.0, 340.0]));
    assert_eq!(hover.hover_viewport, Some(root));
    assert_eq!(hover.samples, 1, "hovering is not a contact sample");
    assert_eq!(hover.last_pen.unwrap().tilt_x, 30.0, "hover keeps the last contact tilt");
    feed.report_contact(root, [220.0, 440.0]);
    assert_eq!(feed.diagnostics().contact, Some([220.0, 440.0]));
    feed.clear_contact(child);
    assert_eq!(feed.diagnostics().contact, Some([220.0, 440.0]), "another viewport cannot clear contact");
    feed.clear_contact(root);
    assert_eq!(feed.diagnostics().contact, None);
    feed.clear_hover(child);
    assert!(feed.diagnostics().hovering, "another viewport cannot clear this hover");
    feed.clear_hover(root);
    let cleared = feed.diagnostics();
    assert!(!cleared.hovering && cleared.hover.is_none());
    assert_eq!(cleared.last_pen, hover.last_pen);
}

#[test]
fn squeeze_wire_name_keeps_legacy_long_press_compatible() {
    for name in ["squeeze", "longPress"] {
        let input: StylusInput = serde_json::from_value(json!({"type":"gesture", "gesture":name, "timestampMs":1000})).unwrap();
        assert!(matches!(input, StylusInput::Gesture { gesture: PenGesture::Squeeze, .. }));
        assert_eq!(serde_json::to_value(input).unwrap()["gesture"], "squeeze");
    }
}

#[test]
fn flash_uses_ui_clock_and_reset_does_not_clear_the_feed() {
    let feed = StylusFeed::default();
    let mut state = DebugUi::default();
    state.set_open(true, &feed.diagnostics());
    feed.push_gesture(PenGesture::Squeeze, 1_800_000_000_000.0);
    state.observe(&feed.diagnostics(), 12.0);
    assert_eq!(state.flashes, [None, Some(12.0)]);
    state.observe(&feed.diagnostics(), 14.0);
    assert_eq!(state.flashes[1], Some(12.0), "repainting must not extend a flash");
    state.reset(&feed.diagnostics());
    assert_eq!(state.baseline[2], 1);
    assert_eq!(state.flashes, [None; 2]);
    assert_eq!(feed.take_gestures(), [PenGesture::Squeeze]);
}

#[test]
fn help_entry_is_available_without_a_document_and_test_mode_blocks_actions() {
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    assert!(crate::menus::invoke(&mut app, &ctx, "help.stylusDebug", json!({"show":"invalid"})).is_err());
    assert!(!app.ui.stylus_debug.open);
    crate::menus::invoke(&mut app, &ctx, "help.stylusDebug", json!({})).unwrap();
    assert!(app.ui.stylus_debug.open && app.ui.stylus_debug.suppress_actions);
    let original = app.ui.tool;
    app.stylus.feed.push_gesture(PenGesture::DoubleTap, 1000.0);
    app.stylus.feed.push_gesture(PenGesture::Squeeze, 2000.0);
    crate::chrome_ui::drain_stylus_gestures(&mut app, &ctx);
    assert_eq!(app.ui.tool, original);
    assert!(app.ui.brush_picker.is_none());
    assert_eq!(app.stylus.feed.diagnostics().squeezes, 1);
    crate::menus::invoke(&mut app, &ctx, "help.stylusDebug", json!({"show":false})).unwrap();
    app.stylus.feed.push_gesture(PenGesture::DoubleTap, 3000.0);
    crate::chrome_ui::drain_stylus_gestures(&mut app, &ctx);
    assert_eq!(app.ui.tool, crate::state::Tool::Eraser, "closing resumes the configured shortcuts");
    assert!(app.session.journal.is_empty());
    let restored: crate::state::UiState = serde_json::from_value(serde_json::to_value(&app.ui).unwrap()).unwrap();
    assert!(!restored.stylus_debug.open);
}

#[test]
fn window_displays_preference_independent_readings_and_can_reset() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.ui.stylus_debug.open = true;
    app.stylus.use_pressure = false;
    let capture = std::env::var_os("PHOTOCRAFT_STYLUS_DEBUG_CAPTURE");
    let builder = Harness::builder().with_size(vec2(1000.0, 900.0));
    let builder = if capture.is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(|ui, app| window(app, ui.ctx()), app);
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ArkuiProLight);
    h.run_steps(4);
    assert!(h.query_by_label("Waiting for pen input").is_some());
    h.state().stylus.feed.set_device(true, Some("Test pen".into()));
    h.state().stylus.feed.report(PointerSource::Pen, Some(PenSample { pressure: 0.7, tilt_x: 45.0, tilt_y: -20.0, ..Default::default() }));
    h.state().stylus.feed.report_hover(h.ctx.viewport_id(), [120.0, 340.0]);
    h.state().stylus.feed.push_gesture(PenGesture::Squeeze, 1000.0);
    h.run_steps(4);
    assert!(h.query_by_label_contains("Tilt X: +45.0°").is_some());
    assert!(h.query_by_label("Received").is_some());
    assert!(h.query_by_label_contains("Hovering").is_some());
    assert!(h.query_by_label_contains("Hovering  X 120  Y 340").is_some());
    assert!(h.query_by_label_contains("Hz").is_some());
    h.state().stylus.feed.clear_hover(h.ctx.viewport_id());
    h.state().stylus.feed.report_contact(h.ctx.viewport_id(), [130.0, 350.0]);
    h.run_steps(3);
    assert!(h.query_by_label_contains("Pen contact  X 130  Y 350").is_some());
    assert!(h.state().stylus.sample().is_none(), "paint pressure off must not hide diagnostic readings");
    if let Some(path) = capture {
        h.render().unwrap().save(path).unwrap();
    }
    h.get_by_label("Reset test").click();
    h.run_steps(8);
    assert_eq!(h.state().ui.stylus_debug.baseline[0], 1, "reset button must run");
    assert!(h.query_by_label("Waiting for pen input").is_some());
    assert!(h.state().session.journal.is_empty());
}
