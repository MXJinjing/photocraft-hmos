use egui::{Event, Modifiers, PointerButton, TouchPhase, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;

use crate::{PhotocraftApp, Services, theme::ThemeKind};

fn app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.session.edit_prefs(|p| p.interface.language = "en".into());
    app
}

fn status(theme: ThemeKind, width: f32, document: bool) -> Harness<'static, PhotocraftApp> {
    let mut app = app();
    if document {
        app.session.execute("file.new", json!({"width": 16, "height": 16})).unwrap();
        app.sync_views();
    }
    app.ui.status = "A long status message ".repeat(100);
    let mut h = Harness::builder().with_size(vec2(width, 100.0)).build_ui_state(
        |ui, app| {
            if !ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                return;
            }
            crate::panels::status_bar(app, ui);
            egui::CentralPanel::default().show(ui, |_| {});
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, theme);
    h.run_steps(4);
    h
}

#[test]
fn controls_stay_at_the_right_edge_with_long_status_text_and_no_document() {
    for theme in ThemeKind::ALL {
        for width in [360.0, 1400.0] {
            for document in [false, true] {
                let h = status(theme, width, document);
                let canvas = h.get_by_label("Only show canvas").rect();
                let dock = h.get_by_label("Right sidebar").rect();
                let bar = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("status_bar")).unwrap().outer_rect;
                assert!(canvas.right() <= dock.left(), "the buttons must not overlap");
                assert!((dock.right() - (bar.right() - 10.0)).abs() < 1.0, "right edge: {dock:?}, bar: {bar:?}");
                assert!((dock.bottom() - bar.bottom()).abs() <= 1.0, "bottom edge: {dock:?}, bar: {bar:?}");
                assert!(bar.contains_rect(canvas) && bar.contains_rect(dock), "hit targets fit inside the status bar");
                assert!(canvas.left() >= 0.0);
                let pro = crate::theme::Tokens::get(&h.ctx).pro;
                let height = if pro { 24.0 } else { 30.0 };
                assert_eq!(bar.height(), height, "{theme:?}: keep the original status bar height");
                assert_eq!(canvas.size(), vec2(height - 2.0, height - 2.0));
                assert_eq!(dock.size(), canvas.size());
            }
        }
    }
}

#[test]
fn fit_sits_beside_zoom_and_works_in_every_theme() {
    for theme in ThemeKind::ALL {
        let mut h = status(theme, 360.0, true);
        let fit = h.get_by_label("Fit").rect();
        let zoom = h.get_by_role(egui::accesskit::Role::SpinButton).rect();
        let bar = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("status_bar")).unwrap().outer_rect;
        assert!(fit.left() > zoom.right() && fit.left() - zoom.right() <= 32.0, "{theme:?}: Fit is immediately right of zoom");
        assert!(bar.contains_rect(fit), "{theme:?}: Fit is visible within the original bar height");
        assert_eq!(fit.height(), crate::panels::status_bar_heights(&crate::theme::Tokens::get(&h.ctx)).1, "{theme:?}: Fit matches the zoom field height");
        assert_eq!(fit.center().y, bar.center().y, "{theme:?}: Fit is vertically centred in the status bar");
        let label_width = h.ctx.fonts_mut(|fonts| fonts.layout_no_wrap("Fit".into(), crate::theme::medium(13.0), egui::Color32::WHITE).size().x);
        assert!((fit.width() - (label_width + 28.0)).abs() < 1.0, "{theme:?}: Fit keeps the original label size and horizontal padding");
        assert_eq!(h.query_all_by_label("Fit").count(), 1, "{theme:?}: one Fit button");
        h.state_mut().ui.views[0].fit_pending = false;
        h.get_by_label("Fit").click();
        h.run_steps(3);
        assert!(h.state().ui.views[0].fit_pending, "{theme:?}: Fit still fits the document");
    }
}

#[test]
fn buttons_restore_the_previous_panel_combination_and_follow_commands() {
    for theme in [ThemeKind::Studio, ThemeKind::ProMedium] {
        let mut h = status(theme, 360.0, true);
        h.state_mut().ui.panels.options_bar = false;
        h.get_by_label("Only show canvas").click();
        h.run_steps(3);
        let p = &h.state().ui.panels;
        assert!(!p.toolbar && !p.options_bar && !p.dock && p.status_bar);
        h.get_by_label("Only show canvas").click();
        h.run_steps(3);
        let p = &h.state().ui.panels;
        assert!(p.toolbar && !p.options_bar && p.dock, "restore only the panels hidden by Tab");
        h.get_by_label("Right sidebar").click();
        h.run_steps(3);
        let p = &h.state().ui.panels;
        assert!(p.toolbar && !p.options_bar && !p.dock && p.status_bar);
        h.get_by_label("Right sidebar").click();
        h.run_steps(3);
        assert!(h.state().ui.panels.dock);
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "window.togglePanels", json!({})).unwrap();
        h.run_steps(3);
        assert!(!h.state().ui.panels.toolbar && !h.state().ui.panels.dock, "keyboard commands still toggle the panels");
        assert_eq!(h.query_all_by_label("Only show canvas").count(), 1, "the button label stays fixed after toggling");
        assert_eq!(h.query_all_by_label("Right sidebar").count(), 1);
    }
}

#[test]
fn touch_taps_work_in_the_full_shell_with_finger_painting_blocked() {
    let mut h = Harness::builder().with_size(vec2(1280.0, 800.0)).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, ThemeKind::ProMedium);
        let mut app = app();
        app.session.execute("file.new", json!({"width": 16, "height": 16})).unwrap();
        app.session.edit_prefs(|p| p.tools.block_finger_input = true);
        app.stylus.feed.report(crate::stylus::PointerSource::Touch, None);
        app
    });
    h.run_steps(4);
    for (label, expected) in [
        ("Only show canvas", (false, false, false)),
        ("Only show canvas", (true, true, true)),
        ("Right sidebar", (true, true, false)),
        ("Right sidebar", (true, true, true)),
    ] {
        let pos = h.get_by_label(label).rect().center();
        h.input_mut().events.extend([
            Event::PointerMoved(pos),
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
            Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(1), phase: TouchPhase::Start, pos, force: Some(1.0) },
        ]);
        h.run_steps(1);
        h.input_mut().events.extend([
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
            Event::Touch { device_id: egui::TouchDeviceId(0), id: egui::TouchId(1), phase: TouchPhase::End, pos, force: Some(1.0) },
            Event::PointerGone,
        ]);
        h.run_steps(3);
        let p = &h.state().ui.panels;
        assert_eq!((p.toolbar, p.options_bar, p.dock), expected, "{label}: touch toggles the same panels as the shortcut");
        assert!(h.state().ui.panels.status_bar, "touch users can always restore the panels");
        assert!(!h.state().session.active().unwrap().history.can_undo(), "status buttons never paint");
    }
}

#[test]
fn simplified_chinese_panel_actions_are_translated() {
    let zh = crate::i18n::Lang::from_code("zh-hans").unwrap();
    for (source, translated) in [("Only show canvas", "只显示画布"), ("Right sidebar", "右侧面板")] {
        assert_eq!(crate::i18n::tr(zh, source), translated);
    }
}
