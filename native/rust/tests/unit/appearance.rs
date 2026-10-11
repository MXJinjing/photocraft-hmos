use super::{Appearance, parse_token};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn tokens_map_to_egui_themes() {
    assert_eq!(parse_token("dark"), Some(egui::Theme::Dark));
    assert_eq!(parse_token("Light"), Some(egui::Theme::Light));
    assert_eq!(parse_token("DARK"), Some(egui::Theme::Dark));
    assert_eq!(parse_token(""), None);
    assert_eq!(parse_token("auto"), None);
    assert_eq!(parse_token("not-a-theme"), None);
}

#[test]
fn publish_updates_service_and_wakes_ui() {
    let appearance = Appearance::from_token("dark");
    let service = appearance.service();
    let ctx = egui::Context::default();
    let repaints = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&repaints);
    ctx.set_request_repaint_callback(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(service(&ctx), Some(egui::Theme::Dark));
    assert_eq!(appearance.current(), Some(egui::Theme::Dark));

    appearance.publish_token("light");
    assert_eq!(service(&ctx), Some(egui::Theme::Light));
    assert_eq!(repaints.load(Ordering::Relaxed), 1);

    appearance.publish_token("light");
    assert_eq!(repaints.load(Ordering::Relaxed), 1, "identical value must not wake again");

    appearance.publish_token("");
    assert_eq!(service(&ctx), None);
    assert_eq!(appearance.current(), None);
    // egui may coalesce a second request_repaint while one is already pending.
}

#[test]
fn auto_appearance_follows_published_system_theme() {
    use photocraft_engine::prefs::{AppearanceMode, DarkTheme, LightTheme};
    use photocraft_ui_egui::{PhotocraftApp, Services, theme::ThemeKind};

    let appearance = Appearance::from_token("light");
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(
        photocraft_engine::Session::new(),
        Services {
            system_theme: Some(appearance.service()),
            ..Default::default()
        },
    );
    app.run(
        "prefs.set",
        serde_json::json!({
            "values": {
                "interface.appearanceMode": "auto",
                "interface.darkTheme": "studio",
                "interface.lightTheme": "classic"
            }
        }),
    )
    .unwrap();
    assert_eq!(
        app.session.prefs().interface.appearance_mode,
        AppearanceMode::Auto
    );
    assert_eq!(app.session.prefs().interface.dark_theme, DarkTheme::Studio);
    assert_eq!(
        app.session.prefs().interface.light_theme,
        LightTheme::Classic
    );

    // sync_appearance runs during host_logic / prefs tick.
    let mut raw = egui::RawInput {
        system_theme: appearance.current(),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        ..Default::default()
    };
    app.host_input(&ctx, &mut raw);
    crate::editor_frame(&ctx, &mut app, raw).drop_without_applying_deltas();
    assert_eq!(app.ui.theme, ThemeKind::Classic);

    appearance.publish_token("dark");
    let mut raw = egui::RawInput {
        system_theme: appearance.current(),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        ..Default::default()
    };
    app.host_input(&ctx, &mut raw);
    crate::editor_frame(&ctx, &mut app, raw).drop_without_applying_deltas();
    assert_eq!(app.ui.theme, ThemeKind::Studio);
}
