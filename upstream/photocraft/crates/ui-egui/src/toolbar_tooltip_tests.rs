use crate::{PhotocraftApp, icons, panels, theme, widgets};
use egui::{Pos2, Rect, vec2};

fn text_in(shape: &egui::Shape, expected: &str) -> bool {
    match shape {
        egui::Shape::Text(text) => text.galley.text() == expected,
        egui::Shape::Vec(shapes) => shapes.iter().any(|shape| text_in(shape, expected)),
        _ => false,
    }
}

/// Exercise real hover frames, including the delay: accessible names alone do not prove that
/// a user can see a tooltip, especially when a button is disabled with no document open.
fn assert_hover_tip(kind: theme::ThemeKind, expected: &str, mut draw: impl FnMut(&mut egui::Ui) -> Rect) {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, kind);
    let mut rect = Rect::NOTHING;
    let mut visible = false;
    for step in 0..16 {
        let events = if step == 2 { vec![egui::Event::PointerMoved(rect.center())] } else { vec![] };
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(f64::from(step) * 0.1),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 1800.0))),
                events,
                ..Default::default()
            },
            |ui| rect = draw(ui),
        );
        output.textures_delta.clear();
        visible |= output.shapes.iter().any(|shape| text_in(&shape.shape, expected));
    }
    assert!(visible, "{kind:?}: missing visible tooltip {expected:?}");
}

#[test]
fn toolbar_icon_tooltips_work_in_every_pro_palette_even_when_disabled() {
    for kind in [
        theme::ThemeKind::Pro,
        theme::ThemeKind::ProMedium,
        theme::ThemeKind::ProLightGray,
        theme::ThemeKind::ProLight,
        theme::ThemeKind::ArkuiProLight,
        theme::ThemeKind::ArkuiProDark,
    ] {
        for enabled in [true, false] {
            assert_hover_tip(kind, "Create a new layer", |ui| {
                ui.add_enabled_ui(enabled, |ui| icons::button(ui, "square-plus", 26.0, false, "Create a new layer")).inner.rect
            });
        }
        let mut selected = 0;
        assert_hover_tip(kind, "Panel menu", |ui| widgets::card_ex(ui, "test-card", &["Layers"], &mut selected, false, false, |_, _| {}).menu.rect);
        let mut color = egui::Color32::RED;
        assert_hover_tip(kind, "Open the Color Picker", |ui| widgets::color_edit_button_srgba(ui, &mut color).rect);
    }
}

#[test]
fn toolbar_column_toggle_explains_both_actions() {
    for double in [false, true] {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        app.ui.panels.toolbar_double = double;
        assert_hover_tip(theme::ThemeKind::Pro, if double { "Show tools in one column" } else { "Show tools in two columns" }, |ui| {
            panels::toolbar(&mut app, ui);
            // The toolbar's fixed header is above its scrolling tool grid.
            Rect::from_min_size(egui::pos2(5.0, 8.0), vec2(30.0, 14.0))
        });
    }
}

#[test]
fn capture_pro_hover_tooltip() {
    let Some(dir) = std::env::var_os("PHOTOCRAFT_TOOLTIP_CAPTURES").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for kind in [theme::ThemeKind::Pro, theme::ThemeKind::ProLight] {
        let mut harness = egui_kittest::Harness::builder().with_size(vec2(400.0, 180.0)).wgpu().build_ui(move |ui| {
            theme::apply(ui.ctx(), kind);
            ui.ctx().global_style_mut(|style| style.interaction.tooltip_delay = 0.0);
            egui_extras::install_image_loaders(ui.ctx());
            ui.add_enabled_ui(false, |ui| icons::button(ui, "square-plus", 26.0, false, "Create a new layer"));
        });
        harness.run_steps(3);
        harness.render().unwrap().save(dir.join(format!("{}-before.png", kind.id()))).unwrap();
        harness.hover_at(egui::pos2(20.0, 20.0));
        harness.run_steps(6);
        harness.render().unwrap().save(dir.join(format!("{}-hover.png", kind.id()))).unwrap();
    }
}

fn device_frame(ctx: &egui::Context, source: crate::stylus::PointerSource, time: f64, events: Vec<egui::Event>, enabled: bool) -> (bool, bool) {
    let mut clicked = false;
    let mut output = ctx.run_ui(
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 180.0))), time: Some(time), events, ..Default::default() },
        |ui| {
            crate::tooltips::sync(ui.ctx(), source, enabled);
            clicked = icons::button(ui, "square-plus", 26.0, false, "Create a new layer").clicked();
        },
    );
    output.textures_delta.clear();
    (output.shapes.iter().any(|shape| text_in(&shape.shape, "Create a new layer")), clicked)
}

#[test]
fn pen_hover_shows_tooltip_immediately_while_moving() {
    use crate::stylus::PointerSource::Pen;
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, theme::ThemeKind::Pro);
    device_frame(&ctx, Pen, 0.0, vec![], true);
    device_frame(&ctx, Pen, 0.01, vec![], true);
    let (shown, clicked) = device_frame(&ctx, Pen, 0.02, vec![egui::Event::PointerMoved(egui::pos2(20.0, 20.0))], true);
    assert!(
        shown || device_frame(&ctx, Pen, 0.021, vec![], true).0,
        "pen hover must paint after its sizing pass, without waiting for the normal hover delay or stillness"
    );
    assert!(!clicked);
    assert!(!device_frame(&ctx, Pen, 0.03, vec![egui::Event::PointerGone], true).0);
    assert!(!device_frame(&ctx, Pen, 0.04, vec![egui::Event::PointerMoved(egui::pos2(20.0, 20.0))], false).0);
}

#[test]
fn touch_tap_shows_hint_after_pointer_gone_without_eating_the_click() {
    use crate::stylus::PointerSource::Touch;
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, theme::ThemeKind::Pro);
    let at = egui::pos2(20.0, 20.0);
    let pointer = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    device_frame(&ctx, Touch, 0.0, vec![], true);
    device_frame(&ctx, Touch, 0.01, vec![], true);
    assert!(!device_frame(&ctx, Touch, 0.1, vec![egui::Event::PointerMoved(at), pointer(true)], true).0);
    let (shown, clicked) = device_frame(&ctx, Touch, 0.2, vec![pointer(false), egui::Event::PointerGone], true);
    assert!(clicked, "a tap must still activate the button");
    // Tooltips may need a sizing pass before their text is painted.
    assert!(shown || device_frame(&ctx, Touch, 0.21, vec![], true).0);
    assert!(device_frame(&ctx, Touch, 1.0, vec![], true).0, "finger lift must not remove the hint");
    assert!(!device_frame(&ctx, Touch, 2.3, vec![], true).0, "hint must expire");
}

#[test]
fn touch_tap_explains_disabled_icon_without_eating_other_input() {
    use crate::stylus::PointerSource::Touch;
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, theme::ThemeKind::Pro);
    let at = egui::pos2(20.0, 20.0);
    let pointer = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let mut shown = false;
    let mut clicked = false;
    for (time, events) in
        [(0.0, vec![]), (0.1, vec![egui::Event::PointerMoved(at), pointer(true)]), (0.2, vec![pointer(false), egui::Event::PointerGone]), (0.21, vec![])]
    {
        let mut output = ctx.run_ui(
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 180.0))), time: Some(time), events, ..Default::default() },
            |ui| {
                crate::tooltips::sync(ui.ctx(), Touch, true);
                clicked = ui.add_enabled_ui(false, |ui| icons::button(ui, "square-plus", 26.0, false, "Create a new layer")).inner.clicked();
            },
        );
        output.textures_delta.clear();
        shown |= output.shapes.iter().any(|shape| text_in(&shape.shape, "Create a new layer"));
    }
    assert!(shown, "a disabled toolbar button still needs a touch hint");
    assert!(!clicked, "explaining a disabled button must not activate it");
}

#[test]
fn pen_delay_is_zero_and_tooltip_preferences_still_apply() {
    use crate::stylus::PointerSource;
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    for source in [PointerSource::Pen, PointerSource::Mouse, PointerSource::Touch] {
        app.stylus.feed.report(source, None);
        crate::prefs_ui::tick(&mut app, &ctx);
        assert_eq!(ctx.global_style().interaction.tooltip_delay, if source == PointerSource::Pen { 0.0 } else { theme::TOOLTIP_DELAY });
        assert_eq!(ctx.global_style().interaction.show_tooltips_only_when_still, source != PointerSource::Pen);
    }
    app.stylus.feed.report(PointerSource::Pen, None);
    app.run("prefs.set", serde_json::json!({"values": {"tools.showTooltips": false}})).unwrap();
    crate::prefs_ui::tick(&mut app, &ctx);
    assert!(ctx.global_style().interaction.tooltip_delay.is_infinite());
}

#[test]
fn pressure_size_button_keeps_its_short_label() {
    use egui_kittest::kittest::Queryable;
    let mut ready = false;
    let capture = std::env::var_os("PHOTOCRAFT_TOOLTIP_CAPTURES").map(std::path::PathBuf::from);
    let builder = egui_kittest::Harness::builder().with_size(vec2(1400.0, 200.0));
    let builder = if capture.is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        move |ui, app: &mut PhotocraftApp| {
            if !ready {
                PhotocraftApp::setup_context(ui.ctx(), theme::ThemeKind::Pro);
                ready = true;
            }
            crate::tooltips::sync(ui.ctx(), crate::stylus::PointerSource::Pen, true);
            panels::options_bar(app, ui);
        },
        PhotocraftApp::new(photocraft_engine::Session::new(), Default::default()),
    );
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    h.run_steps(3);
    h.get_by_label("Always use pressure for size").hover();
    h.run_steps(2);
    assert!(
        h.output().shapes.iter().any(|shape| text_in(&shape.shape, "Always use pressure for size")),
        "the pressure-size hint should keep its existing short label"
    );
    if let Some(dir) = capture {
        h.render().unwrap().save(dir.join("pressure-size-pro-hover.png")).unwrap();
    }
}
