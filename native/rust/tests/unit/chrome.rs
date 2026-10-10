use super::*;

#[test]
fn titlebar_geometry_converts_pixels_and_rejects_bad_scale() {
    assert_eq!(titlebar_geometry(120.0, 40.0, 2.0), egui::vec2(60.0, 20.0));
    assert_eq!(titlebar_geometry(-1.0, -2.0, 2.0), egui::Vec2::ZERO);
    assert_eq!(titlebar_geometry(120.0, 40.0, 0.0), egui::Vec2::ZERO);
}
use photocraft_ui_egui::{PhotocraftApp, Services};

#[test]
fn persisted_theme_and_live_changes_reach_the_host_after_logic() {
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(
        photocraft_engine::Session::new(),
        Services {
            load_prefs: Some(Box::new(|| {
                Some(r#"{"interface":{"theme":"studioLight"}}"#.into())
            })),
            ..Default::default()
        },
    );
    let mut sync = ChromeSync::default();
    let raw = egui::RawInput::default();
    let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
    assert_eq!(sync.update(app.ui.theme).as_deref(), Some("#F6F6F8"));
    assert_eq!(sync.update(app.ui.theme), None);
    app.run(
        "prefs.set",
        serde_json::json!({"values":{"interface.theme":"studio"}}),
    )
    .unwrap();
    let _ = ctx.run_logic(&raw, |ctx| app.host_logic(ctx));
    assert_eq!(sync.update(app.ui.theme).as_deref(), Some("#141415"));
    sync.invalidate(); // A new surface or foreground transition must repaint the window.
    assert_eq!(sync.update(app.ui.theme).as_deref(), Some("#141415"));
    assert_eq!(sync.update(app.ui.theme), None);
}
