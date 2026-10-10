use super::*;
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::json;

#[test]
fn output_profile_tracks_native_declaration_without_changing_document() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", json!({"width": 16, "height": 16}))
        .unwrap();
    app.run("edit.assignProfile", json!({"profile": "display-p3"}))
        .unwrap();
    let original = app.session.active().unwrap().doc.icc_profile.clone();
    DisplayColor::new(true)
        .unwrap()
        .install(&mut app.session.color);
    let doc = &app.session.active().unwrap().doc;
    assert!(app.session.color.canvas_display(doc).unwrap().is_identity());
    let status = app.session.color.monitor_status();
    assert_eq!(status.source, "auto");
    assert!(status.profile_name.unwrap().contains("HarmonyOS output"));
    DisplayColor::new(false)
        .unwrap()
        .install(&mut app.session.color);
    let doc = &app.session.active().unwrap().doc;
    assert!(!app.session.color.canvas_display(doc).unwrap().is_identity());
    assert_eq!(doc.icc_profile, original);
    assert_eq!(
        app.session.color.monitor().content_hash(),
        Builtin::Srgb.profile().content_hash()
    );
}

#[test]
fn srgb_ui_maps_into_p3_and_preserves_alpha_and_neutrals() {
    let mut p3 = DisplayColor::new(true).unwrap();
    let red = p3.ui_color(Color32::RED);
    // Published sRGB -> Display P3 conversion: red is approximately (234, 51, 35).
    for (actual, expected) in red.to_array()[..3].iter().zip([234i16, 51, 35]) {
        assert!((i16::from(*actual) - expected).abs() <= 2);
    }
    for alpha in [0, 1, 64, 128, 255] {
        let color = Color32::from_rgba_unmultiplied(230, 40, 70, alpha);
        assert_eq!(p3.ui_color(color).a(), alpha);
        let gray = Color32::from_rgba_unmultiplied(100, 100, 100, alpha);
        assert_eq!(p3.ui_color(gray), gray);
    }
    let mut srgb = DisplayColor::new(false).unwrap();
    assert_eq!(srgb.ui_color(Color32::RED), Color32::RED);
}

#[test]
fn tagged_canvas_is_not_converted_twice_and_partial_ui_updates_are_converted() {
    let ctx = Context::default();
    let pixels = egui::ColorImage::filled([2, 2], Color32::RED);
    let managed = ctx.load_texture(
        format!(
            "{}canvas-test",
            photocraft_ui_egui::canvas::DISPLAY_TEXTURE_PREFIX
        ),
        pixels.clone(),
        egui::TextureOptions::LINEAR,
    );
    let ui = ctx.load_texture("ordinary-ui", pixels.clone(), egui::TextureOptions::LINEAR);
    let mut p3 = DisplayColor::new(true).unwrap();
    let mut managed_image: egui::ImageData = pixels.clone().into();
    p3.texture(&ctx, managed.id(), &mut managed_image);
    assert!(managed_image == egui::ImageData::from(pixels));
    let mut partial: egui::ImageData = egui::ColorImage::filled([1, 1], Color32::RED).into();
    p3.texture(&ctx, ui.id(), &mut partial);
    let egui::ImageData::Color(img) = partial;
    assert_eq!(img.pixels, vec![p3.ui_color(Color32::RED)]);
}

#[test]
fn actual_cpu_canvas_is_tagged_as_display_pixels() {
    let ctx = Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", json!({"width": 16, "height": 16}))
        .unwrap();
    DisplayColor::new(true)
        .unwrap()
        .install(&mut app.session.color);
    let (texture, _) =
        photocraft_ui_egui::canvas::ensure_texture(&mut app, &ctx, 0, Some(1)).unwrap();
    assert!(
        ctx.tex_manager()
            .read()
            .meta(texture)
            .unwrap()
            .name
            .starts_with(photocraft_ui_egui::canvas::DISPLAY_TEXTURE_PREFIX)
    );
}

#[test]
fn old_host_config_defaults_to_srgb() {
    let cfg: crate::Config = serde_json::from_str(r#"{"files":"/tmp","scale":1}"#).unwrap();
    assert!(!cfg.display_p3);
}
