//! Input routing through the real canvas, dialogs and shortcuts (#44, #45): keyboard zoom hits the
//! canvas (never egui's UI scale), and an open dialog keeps the canvas pannable and zoomable but
//! is not cancelled by a click outside it.

use egui::{Key, Modifiers, Pos2, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

use crate::PhotocraftApp;

fn harness() -> Harness<'static, PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app.sync_views();
    let mut h = Harness::builder().with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut PhotocraftApp| {
            let ctx = ui.ctx().clone();
            // Fonts set up after the first frame only apply from the next one.
            if !ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                return;
            }
            crate::shortcuts::handle(app, &ctx);
            egui::CentralPanel::default().show(ui, |ui| crate::canvas::document_area(app, ui));
            crate::dialogs::show(app, &ctx);
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
    h.run_steps(4);
    h
}

fn zoom(h: &Harness<'static, PhotocraftApp>) -> f32 {
    h.state().current_zoom()
}

fn open_dialog(h: &mut Harness<'static, PhotocraftApp>) {
    crate::dialogs::open_command_dialog(h.state_mut(), "image.adjustments.brightnessContrast", "Brightness/Contrast…");
    h.run_steps(3);
    assert_eq!(h.state().ui.dialogs.len(), 1);
}

/// A point on the canvas well away from the centred dialog.
fn free_canvas(h: &Harness<'static, PhotocraftApp>) -> Pos2 {
    let r = h.state().last_canvas_rect;
    pos2(r.left() + 60.0, r.bottom() - 60.0)
}

#[test]
fn ctrl_plus_zooms_the_canvas_not_the_interface() {
    let mut h = harness();
    let z0 = zoom(&h);
    // ⌘+ / Ctrl+'+' as typed with ⇧ on a US layout, and as the numpad / Nordic `+`.
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Plus);
    h.run_steps(2);
    let z1 = zoom(&h);
    assert!(z1 > z0, "⌘+ should zoom the canvas in ({z0} -> {z1})");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Plus);
    h.run_steps(2);
    assert!(zoom(&h) > z1);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Minus);
    h.run_steps(2);
    assert!((zoom(&h) - z1).abs() < 1e-4);
    // ⌘1 is 100%, ⌘0 fits on screen again.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    h.run_steps(2);
    assert_eq!(zoom(&h), 1.0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num0);
    h.run_steps(3);
    assert!((zoom(&h) - z0).abs() < 1e-3, "⌘0 fits on screen: {} vs {z0}", zoom(&h));
    assert_eq!(h.ctx.zoom_factor(), 1.0, "the interface must never scale");
}

#[test]
fn keyboard_zoom_works_with_a_dialog_open() {
    let mut h = harness();
    open_dialog(&mut h);
    let z0 = zoom(&h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
    h.run_steps(2);
    assert!(zoom(&h) > z0);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Plus);
    h.run_steps(2);
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    assert_eq!(h.state().ui.dialogs.len(), 1);
    // Other commands stay blocked while the dialog is open (⌘J would duplicate the layer).
    let layers = h.state().session.active().unwrap().doc.layers.len();
    h.key_press_modifiers(Modifiers::COMMAND, Key::J);
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().doc.layers.len(), layers);
}

#[test]
fn clicking_outside_a_dialog_keeps_it_open_and_the_canvas_pans_and_zooms() {
    let mut h = harness();
    open_dialog(&mut h);
    let p = free_canvas(&h);
    h.hover_at(p);
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    assert_eq!(h.state().ui.dialogs.len(), 1, "a click outside must not cancel the dialog");

    // Scrolling over the canvas pans it.
    let c0 = h.state().ui.views[0].center;
    h.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -40.0), phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE });
    h.run_steps(8);
    let c1 = h.state().ui.views[0].center;
    assert!(c1[1] > c0[1], "scroll pans under a dialog: {c0:?} -> {c1:?}");

    // Space-drag pans too.
    h.event(egui::Event::Key { key: Key::Space, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for i in 1..=5 {
        h.hover_at(p + vec2(10.0 * i as f32, 0.0));
        h.run_steps(1);
    }
    h.event(egui::Event::PointerButton { pos: p + vec2(50.0, 0.0), button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.event(egui::Event::Key { key: Key::Space, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    let c2 = h.state().ui.views[0].center;
    let z = zoom(&h);
    assert!((c2[0] - (c1[0] - 50.0 / z)).abs() < 0.5, "space-drag pans by 50 pt: {c1:?} -> {c2:?}");
    assert_eq!(h.state().ui.dialogs.len(), 1);

    // Esc still cancels.
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().ui.dialogs.is_empty());
}

fn press(h: &mut Harness<'static, PhotocraftApp>, p: Pos2, pressed: bool) {
    h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    h.run_steps(1);
}

fn picker_color(h: &Harness<'static, PhotocraftApp>) -> String {
    let d = h.state().ui.dialogs.last().unwrap();
    d.fields.get("color").and_then(serde_json::Value::as_str).unwrap().to_string()
}

/// The Color Picker's eyedropper, as in Photoshop: over the image the pointer is a pipette, and a
/// click or drag there samples into the picker's new colour, whatever the tool. OK applies it.
#[test]
fn color_picker_samples_the_image_under_its_pipette() {
    let mut h = harness();
    h.state_mut().run("shape.create", json!({"kind": "rect", "rect": [0, 0, 200, 300], "fill": "#ff0000"})).unwrap();
    h.state_mut().run("shape.create", json!({"kind": "rect", "rect": [200, 0, 200, 300], "fill": "#00ff00"})).unwrap();
    // 400 %: the image covers the whole canvas, red on the left of the dialog, green on its right.
    let v = &mut h.state_mut().ui.views[0];
    (v.zoom, v.center, v.fit_pending) = (4.0, [200.0, 150.0], false);
    crate::color_picker_ui::open(h.state_mut(), "foreground");
    h.run_steps(3);
    let r = h.state().last_canvas_rect;
    let (red, green) = (pos2(r.left() + 60.0, r.center().y), pos2(r.right() - 60.0, r.center().y));
    assert_eq!(doc_at(&h, red)[0] < 200.0, doc_at(&h, green)[0] > 200.0);
    let foreground = h.state().session.tools.foreground;

    h.hover_at(red);
    h.run_steps(1);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::None, "the pipette replaces the pointer");
    h.hover_at(r.center());
    h.run_steps(1);
    assert_ne!(h.output().platform_output.cursor_icon, egui::CursorIcon::None, "over the dialog it is the normal pointer");

    // A click samples; the press and release may land in one frame (`ui.click`).
    h.hover_at(red);
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: red, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    press(&mut h, red, false);
    assert_eq!(picker_color(&h), "#ff0000");
    assert_eq!(h.state().session.tools.foreground, foreground, "only OK sets the foreground");

    // A drag keeps sampling, skipping the dialog on the way.
    press(&mut h, red, true);
    for i in 1..=6 {
        h.hover_at(red + (green - red) * (i as f32 / 6.0));
        h.run_steps(1);
    }
    press(&mut h, green, false);
    assert_eq!(picker_color(&h), "#00ff00");

    // Space-drag still pans instead of sampling.
    h.hover_at(red);
    h.run_steps(1);
    let c0 = h.state().ui.views[0].center;
    h.event(egui::Event::Key { key: Key::Space, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(1);
    press(&mut h, red, true);
    h.hover_at(red + vec2(40.0, 0.0));
    h.run_steps(1);
    press(&mut h, red + vec2(40.0, 0.0), false);
    h.event(egui::Event::Key { key: Key::Space, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    let c1 = h.state().ui.views[0].center;
    assert!((c1[0] - (c0[0] - 10.0)).abs() < 0.5, "space-drag pans by 40 pt at 400 %: {c0:?} -> {c1:?}");
    assert_eq!(picker_color(&h), "#00ff00");

    // With the Hand tool a click samples too (the tool doesn't matter); OK applies the sample.
    h.state_mut().ui.tool = crate::state::Tool::Hand;
    h.hover_at(red);
    h.run_steps(1);
    press(&mut h, red, true);
    press(&mut h, red, false);
    assert_eq!(picker_color(&h), "#ff0000");
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(h.state().ui.dialogs.is_empty());
    assert_eq!(h.state().session.tools.foreground, [1.0, 0.0, 0.0, 1.0]);
}

/// Type tool (#206): Alt+←/→ at a collapsed caret kerns the pair before it by 20/1000 em (100
/// with ⌘/Ctrl), one history step per press; ⌘/Ctrl+←/→ moves by word; Alt+Shift+→ extends
/// the selection by a word.
fn wheel(h: &Harness<'static, PhotocraftApp>, dy: f32, modifiers: Modifiers) {
    h.event_modifiers(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: vec2(0.0, dy), phase: egui::TouchPhase::Move, modifiers }, modifiers);
}

/// The document point under screen point `p` (no flip).
fn doc_at(h: &Harness<'static, PhotocraftApp>, p: Pos2) -> [f32; 2] {
    let v = &h.state().ui.views[0];
    let c = h.state().last_canvas_rect.center();
    [v.center[0] + (p.x - c.x) / v.zoom, v.center[1] + (p.y - c.y) / v.zoom]
}

#[test]
fn alt_scroll_zooms_gently_around_the_pointer() {
    let mut h = harness();
    let r = h.state().last_canvas_rect;
    let p = pos2(r.center().x + 120.0, r.center().y - 70.0);
    h.hover_at(p);
    h.run_steps(2);
    let (z0, d0) = (zoom(&h), doc_at(&h, p));
    // One notch with ⌥ held: +5%, the point under the pointer stays put. The modifiers are
    // released right after the event, while egui still smooths the notch over later frames.
    wheel(&h, 1.0, Modifiers::ALT);
    h.run_steps(40);
    let (z1, d1) = (zoom(&h), doc_at(&h, p));
    assert!((z1 / z0 - 1.05).abs() < 1e-3, "one ⌥ notch is 5%: {z0} -> {z1}");
    assert!((d1[0] - d0[0]).abs() < 0.05 && (d1[1] - d0[1]).abs() < 0.05, "centred on the pointer: {d0:?} -> {d1:?}");
    // Three notches back out.
    wheel(&h, -3.0, Modifiers::ALT);
    h.run_steps(40);
    assert!((zoom(&h) / z1 - 1.05f32.powi(-3)).abs() < 1e-3, "{z1} -> {}", zoom(&h));

    // A plain notch still pans, and does not zoom.
    let (z2, c2) = (zoom(&h), h.state().ui.views[0].center);
    wheel(&h, -1.0, Modifiers::NONE);
    h.run_steps(40);
    assert_eq!(zoom(&h), z2, "a plain scroll never zooms");
    assert!(h.state().ui.views[0].center[1] > c2[1], "a plain scroll pans");

    // ⌘/Ctrl + scroll is still the faster gesture zoom.
    wheel(&h, 1.0, Modifiers::COMMAND);
    h.run_steps(40);
    assert!(zoom(&h) / z2 > 1.05, "⌘-scroll zooms in bigger steps: {z2} -> {}", zoom(&h));
}

#[test]
fn alt_scroll_zooms_while_a_temporary_tool_is_held() {
    let mut h = harness();
    let p = h.state().last_canvas_rect.center();
    h.hover_at(p);
    // Space (the temporary Hand, #249) is down: ⌥ + scroll still zooms by notches, and the
    // held key never changes the current tool.
    h.event(egui::Event::Key { key: Key::Space, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    let z0 = zoom(&h);
    wheel(&h, 2.0, Modifiers::ALT);
    h.run_steps(40);
    assert!((zoom(&h) / z0 - 1.05f32.powi(2)).abs() < 1e-3, "{z0} -> {}", zoom(&h));
    h.event(egui::Event::Key { key: Key::Space, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert_eq!(h.state().ui.tool, crate::state::Tool::Brush);
}

fn touch(id: u64, phase: egui::TouchPhase, pos: Pos2) -> egui::Event {
    egui::Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(id), phase, pos, force: Some(1.0) }
}

fn pointer_button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn open_stylus_menu(h: &mut Harness<'static, PhotocraftApp>, tool: crate::state::Tool) {
    h.state_mut().ui.tool = tool;
    h.state_mut().session.edit_prefs(|p| p.tools.block_finger_input = true);
    h.state_mut().session.edit_prefs(|p| p.tools.stylus_long_press = crate::chrome_ui::StylusLongPress::ContextMenu);
    h.state_mut().stylus.feed.push_gesture(crate::stylus::PenGesture::LongPress, 1_000.0);
    let ctx = h.ctx.clone();
    crate::chrome_ui::drain_stylus_gestures(h.state_mut(), &ctx);
    h.run_steps(3);
    assert!(canvas_menu_open(h.state()), "{tool:?} long-press must open a menu");
    h.state_mut().stylus.feed.report(crate::stylus::PointerSource::Touch, None);
}

fn canvas_menu_open(app: &PhotocraftApp) -> bool {
    app.ui.brush_picker.is_some() || app.ui.canvas_tool_menu.is_some() || app.ui.layer_menu.is_some()
}

fn capture_menu_frame(h: &mut Harness<'static, PhotocraftApp>, name: &str) {
    if let Some(dir) = std::env::var_os("PHOTOCRAFT_INPUT_SCREENSHOT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        h.render().unwrap().save(dir.join(format!("{name}.png"))).unwrap();
    }
}

#[test]
fn finger_dismisses_stylus_menus_and_they_stay_closed_after_lift() {
    use crate::state::Tool;
    for tool in [Tool::Brush, Tool::RectMarquee, Tool::Move] {
        let mut h = harness();
        open_stylus_menu(&mut h, tool);
        capture_menu_frame(&mut h, &format!("{tool:?}-open"));
        let p = free_canvas(&h);
        let revision = h.state().session.active().unwrap().revision;
        h.input_mut().events.extend([egui::Event::PointerMoved(p), pointer_button(p, true), touch(1, egui::TouchPhase::Start, p)]);
        h.run_steps(1);
        assert!(!canvas_menu_open(h.state()), "{tool:?}: finger-down must close the menu, not just hide it");
        // Keep the finger down past egui's long-touch threshold before lifting it.
        h.run_steps(45);
        assert!(!canvas_menu_open(h.state()), "{tool:?}: the dismissing contact must not reopen the menu");
        capture_menu_frame(&mut h, &format!("{tool:?}-finger-down"));
        h.input_mut().events.extend([pointer_button(p, false), touch(1, egui::TouchPhase::End, p)]);
        h.run_steps(4);
        assert!(!canvas_menu_open(h.state()), "{tool:?}: menu must stay closed after finger-up");
        capture_menu_frame(&mut h, &format!("{tool:?}-finger-up"));
        assert_eq!(h.state().session.active().unwrap().revision, revision, "dismissing must not edit the document");
    }
}

#[test]
fn blocked_finger_can_choose_an_item_in_the_stylus_menu() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    open_stylus_menu(&mut h, crate::state::Tool::RectMarquee);
    let p = h.get_by_label("Select All").rect().center();
    h.input_mut().events.extend([egui::Event::PointerMoved(p), pointer_button(p, true), touch(1, egui::TouchPhase::Start, p)]);
    h.run_steps(1);
    assert!(h.query_by_label("Select All").is_some(), "menu items must stay visible while a finger is down");
    h.input_mut().events.extend([pointer_button(p, false), touch(1, egui::TouchPhase::End, p)]);
    h.run_steps(4);
    assert!(!canvas_menu_open(h.state()));
    assert!(h.state().session.active().unwrap().doc.selection.is_some(), "finger tap must run the menu command");
}

#[test]
fn pen_contact_dismisses_menu_without_painting_then_next_contact_paints() {
    let mut h = harness();
    open_stylus_menu(&mut h, crate::state::Tool::Brush);
    h.state_mut().stylus.feed.report(crate::stylus::PointerSource::Pen, Some(crate::stylus::PenSample::default()));
    let p = h.state().last_canvas_rect.center() - vec2(120.0, 30.0);
    let moved = p + vec2(50.0, -20.0);
    let revision = h.state().session.active().unwrap().revision;
    h.event(egui::Event::PointerMoved(p));
    h.event(pointer_button(p, true));
    h.run_steps(1);
    assert!(!canvas_menu_open(h.state()), "pen-down outside the menu closes it");
    h.event(egui::Event::PointerMoved(moved));
    h.run_steps(3);
    assert!(h.state().drag.is_none(), "the dismissing contact must not start a stroke after the menu closes");
    h.event(pointer_button(moved, false));
    h.run_steps(3);
    assert_eq!(h.state().session.active().unwrap().revision, revision, "pen dismissal must not paint");
    // A new contact is no longer owned by the dismissed menu.
    h.event(pointer_button(p, true));
    h.run_steps(1);
    h.event(egui::Event::PointerMoved(moved));
    h.run_steps(1);
    h.event(pointer_button(moved, false));
    h.run_steps(3);
    assert!(h.state().session.active().unwrap().revision > revision, "the next pen contact must paint normally");
}

#[test]
fn two_fingers_pan_the_canvas_and_do_not_paint() {
    let mut h = harness();
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    let _ = h.state_mut().run("tools.setBrush", json!({"brush": {"size": 40, "hardness": 1.0}}));
    let r = h.state().last_canvas_rect;
    let a = r.center();
    let b = a + vec2(48.0, 0.0);
    h.hover_at(a);
    h.run_steps(1);
    let (rev0, c0) = (h.state().session.active().unwrap().revision, h.state().ui.views[0].center);
    h.input_mut().events.extend([pointer_button(a, true), touch(1, egui::TouchPhase::Start, a), touch(2, egui::TouchPhase::Start, b)]);
    h.run_steps(1);
    let moved = a + vec2(36.0, 24.0);
    h.input_mut().events.extend([
        egui::Event::PointerMoved(moved),
        touch(1, egui::TouchPhase::Move, moved),
        touch(2, egui::TouchPhase::Move, b + vec2(36.0, 24.0)),
    ]);
    h.run_steps(2);
    h.input_mut().events.extend([pointer_button(moved, false), touch(1, egui::TouchPhase::End, moved), touch(2, egui::TouchPhase::End, b + vec2(36.0, 24.0))]);
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().revision, rev0, "two fingers must not commit a tool");
    let c1 = h.state().ui.views[0].center;
    assert!((c1[0] - c0[0]).abs() > 1.0 || (c1[1] - c0[1]).abs() > 1.0, "two fingers pan: {c0:?} -> {c1:?}");
}

#[test]
fn wheel_scroll_owns_mixed_pointer_input_and_never_paints() {
    for delta in [vec2(-80.0, 0.0), vec2(80.0, 0.0), vec2(0.0, -80.0), vec2(0.0, 80.0)] {
        for touch_compat in [false, true] {
            let mut h = harness();
            h.state_mut().ui.tool = crate::state::Tool::Brush;
            let p = h.state().last_canvas_rect.center();
            h.hover_at(p);
            h.run_steps(1);
            let revision = h.state().session.active().unwrap().revision;
            let center = h.state().ui.views[0].center;
            // A scroll can arrive with compatibility pointer events or a button still held. It
            // must own the whole gesture, including frames after egui's wheel smoothing settles.
            h.input_mut().events.extend([
                pointer_button(p, true),
                egui::Event::PointerMoved(p + delta),
                egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE },
            ]);
            if touch_compat {
                h.event(touch(1, egui::TouchPhase::Start, p));
            }
            h.run_steps(40);
            assert!(h.state().drag.is_none(), "wheel navigation must never start a brush stroke");
            h.event(egui::Event::PointerMoved(p + delta * 1.5));
            h.run_steps(2);
            h.event(pointer_button(p + delta * 1.5, false));
            if touch_compat {
                h.event(touch(1, egui::TouchPhase::End, p + delta * 1.5));
            }
            h.run_steps(3);
            assert_eq!(h.state().session.active().unwrap().revision, revision, "scrolling {delta:?} must not paint");
            let end = h.state().ui.views[0].center;
            if delta.x != 0.0 {
                assert!((end[0] - center[0]) * delta.x < 0.0, "horizontal wheel must pan: {center:?} -> {end:?}");
                assert_eq!(end[1], center[1]);
            } else {
                assert!((end[1] - center[1]) * delta.y < 0.0, "vertical wheel must pan: {center:?} -> {end:?}");
                assert_eq!(end[0], center[0]);
            }
            // Normal mouse/pen drawing resumes after release.
            h.event(pointer_button(p, true));
            h.event(pointer_button(p, false));
            h.run_steps(3);
            assert!(h.state().session.active().unwrap().revision > revision);
        }
    }
}

#[test]
fn a_second_finger_aborts_an_in_progress_stroke() {
    let mut h = harness();
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    let _ = h.state_mut().run("tools.setBrush", json!({"brush": {"size": 40, "hardness": 1.0}}));
    let r = h.state().last_canvas_rect;
    let a = r.center();
    h.hover_at(a);
    h.run_steps(1);
    let rev0 = h.state().session.active().unwrap().revision;
    h.input_mut().events.extend([pointer_button(a, true), touch(1, egui::TouchPhase::Start, a)]);
    h.run_steps(1);
    h.input_mut().events.extend([egui::Event::PointerMoved(a + vec2(20.0, 0.0)), touch(1, egui::TouchPhase::Move, a + vec2(20.0, 0.0))]);
    h.run_steps(1);
    h.input_mut().events.push(touch(2, egui::TouchPhase::Start, a + vec2(40.0, 8.0)));
    h.run_steps(1);
    h.input_mut().events.extend([
        pointer_button(a + vec2(20.0, 0.0), false),
        touch(1, egui::TouchPhase::End, a + vec2(20.0, 0.0)),
        touch(2, egui::TouchPhase::End, a + vec2(40.0, 8.0)),
    ]);
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().revision, rev0, "the interrupted stroke must not commit");
}

#[test]
fn one_finger_still_paints() {
    let mut h = harness();
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    let _ = h.state_mut().run("tools.setBrush", json!({"brush": {"size": 40, "hardness": 1.0}}));
    let a = h.state().last_canvas_rect.center();
    h.hover_at(a);
    h.run_steps(1);
    let rev0 = h.state().session.active().unwrap().revision;
    h.event(pointer_button(a, true));
    h.event(egui::Event::PointerMoved(a + vec2(40.0, 0.0)));
    h.event(pointer_button(a + vec2(40.0, 0.0), false));
    h.run_steps(2);
    assert!(h.state().session.active().unwrap().revision > rev0, "one finger still paints");
}

#[test]
fn two_fingers_can_pinch_zoom() {
    let mut h = harness();
    let r = h.state().last_canvas_rect;
    let mid = r.center();
    let a = mid - vec2(20.0, 0.0);
    let b = mid + vec2(20.0, 0.0);
    h.hover_at(mid);
    h.run_steps(1);
    let z0 = zoom(&h);
    h.input_mut().events.extend([pointer_button(a, true), touch(1, egui::TouchPhase::Start, a), touch(2, egui::TouchPhase::Start, b)]);
    h.run_steps(1);
    // Spread the fingers: egui's pinch zoom is the change in distance, not Event::Zoom
    // (Event::Zoom is ignored while two touches are down).
    let a2 = mid - vec2(40.0, 0.0);
    let b2 = mid + vec2(40.0, 0.0);
    h.hover_at(mid);
    h.input_mut().events.extend([egui::Event::PointerMoved(a2), touch(1, egui::TouchPhase::Move, a2), touch(2, egui::TouchPhase::Move, b2)]);
    h.run_steps(2);
    assert!(zoom(&h) > z0, "pinch zooms the canvas: {z0} -> {}", zoom(&h));
}

#[test]
fn blocking_finger_input_pans_with_one_finger_and_does_not_paint() {
    let mut h = harness();
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    h.state_mut().session.edit_prefs(|p| p.tools.block_finger_input = true);
    h.state_mut().stylus.feed.report(crate::stylus::PointerSource::Touch, None);
    let _ = h.state_mut().run("tools.setBrush", json!({"brush": {"size": 40, "hardness": 1.0}}));
    let a = h.state().last_canvas_rect.center();
    h.hover_at(a);
    h.run_steps(1);
    let (rev0, c0) = (h.state().session.active().unwrap().revision, h.state().ui.views[0].center);
    h.input_mut().events.extend([pointer_button(a, true), touch(1, egui::TouchPhase::Start, a)]);
    h.run_steps(1);
    let moved = a + vec2(40.0, 16.0);
    h.input_mut().events.extend([egui::Event::PointerMoved(moved), touch(1, egui::TouchPhase::Move, moved)]);
    h.run_steps(2);
    h.input_mut().events.extend([pointer_button(moved, false), touch(1, egui::TouchPhase::End, moved)]);
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().revision, rev0, "a blocked finger must not paint");
    let c1 = h.state().ui.views[0].center;
    assert!((c1[0] - c0[0]).abs() > 1.0 || (c1[1] - c0[1]).abs() > 1.0, "one finger pans: {c0:?} -> {c1:?}");
}

#[test]
fn blocking_finger_input_still_lets_the_pen_paint() {
    let mut h = harness();
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    h.state_mut().session.edit_prefs(|p| p.tools.block_finger_input = true);
    h.state_mut()
        .stylus
        .feed
        .report(crate::stylus::PointerSource::Pen, Some(crate::stylus::PenSample { pressure: 0.8, tilt_x: 0.0, tilt_y: 0.0, rotation: 0.0, eraser: false }));
    let _ = h.state_mut().run("tools.setBrush", json!({"brush": {"size": 40, "hardness": 1.0}}));
    let a = h.state().last_canvas_rect.center();
    h.hover_at(a);
    h.run_steps(1);
    let rev0 = h.state().session.active().unwrap().revision;
    h.event(pointer_button(a, true));
    h.event(egui::Event::PointerMoved(a + vec2(40.0, 0.0)));
    h.event(pointer_button(a + vec2(40.0, 0.0), false));
    h.run_steps(2);
    assert!(h.state().session.active().unwrap().revision > rev0, "the pen still paints");
}

#[test]
fn type_tool_alt_arrows_kern_the_pair() {
    use photocraft_doc::text::Kerning;
    let mut h = harness();
    let id = h.state_mut().run("type.create", json!({"x": 20, "y": 80, "text": "AVA To", "size": 40, "font": "Inter"})).unwrap()["layer"].as_u64().unwrap();
    let text = |h: &Harness<'static, PhotocraftApp>| {
        let st = h.state().session.active().unwrap();
        match &st.doc.layer(photocraft_doc::LayerId(id)).unwrap().content {
            photocraft_doc::LayerContent::Text(t) => t.clone(),
            _ => panic!("not text"),
        }
    };
    let kern = |h: &Harness<'static, PhotocraftApp>| {
        let t = text(h);
        let r = t.char_runs();
        (r[0].style.kerning, r[0].style.kern, r.len())
    };
    let steps = |h: &Harness<'static, PhotocraftApp>| h.state().session.active().unwrap().history.entries().len();
    let metric = photocraft_text::shared().lock().unwrap().pair_kerning(&text(&h), 72.0, 0).unwrap().round();
    h.state_mut().ui.text_edit = Some(crate::state::TextEdit {
        layer: id,
        caret: 1,
        anchor: 1,
        session: "kern-test".into(),
        created: false,
        dragging: false,
        resize: None,
        preedit: None,
    });
    h.run_steps(2);
    let s0 = steps(&h);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(kern(&h).0, Kerning::Off);
    assert_eq!(kern(&h).1, metric + 20.0);
    h.key_press_modifiers(Modifiers::ALT | Modifiers::COMMAND, Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(kern(&h).1, metric + 120.0);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(kern(&h).1, metric + 100.0);
    assert_eq!(steps(&h), s0 + 3, "one history step per press");
    // The caret didn't move; ⌘/Ctrl+→ moves by word, Alt+Shift+→ selects by word.
    assert_eq!(h.state().ui.text_edit.as_ref().map(|e| (e.caret, e.anchor)), Some((1, 1)));
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(h.state().ui.text_edit.as_ref().map(|e| (e.caret, e.anchor)), Some((3, 3)));
    h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(h.state().ui.text_edit.as_ref().map(|e| (e.anchor, e.caret)), Some((3, 6)));
    // With a selection Alt+→ moves by word instead of kerning; at the text end there's no pair.
    let before = kern(&h);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(kern(&h), before);
    assert_eq!(steps(&h), s0 + 3);
    // Undo walks back one press at a time.
    assert!(h.state_mut().session.undo());
    h.run_steps(1);
    assert_eq!(kern(&h).1, metric + 120.0);
}

/// The Character panel's kerning field reads and parses Photoshop's values.
#[test]
fn kerning_field_values() {
    use photocraft_doc::text::Kerning;
    assert_eq!(crate::type_tool::kerning_label((Kerning::Metrics, 0.0)), "Metrics");
    assert_eq!(crate::type_tool::kerning_label((Kerning::Optical, 0.0)), "Optical");
    assert_eq!(crate::type_tool::kerning_label((Kerning::Off, 0.0)), "0");
    assert_eq!(crate::type_tool::kerning_label((Kerning::Off, -49.6)), "-50");
    for (s, v) in [("metrics", Some(json!("metrics"))), (" Optical ", Some(json!("optical"))), ("120", Some(json!(120.0))), ("-25.4", Some(json!(-25.0)))] {
        assert_eq!(crate::type_tool::parse_kerning(s), v, "{s}");
    }
    for s in ["", "tight", "1e9", "-5000", "NaN", "inf"] {
        assert_eq!(crate::type_tool::parse_kerning(s), None, "{s}");
    }
}
