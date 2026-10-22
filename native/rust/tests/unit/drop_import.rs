use super::*;
use photocraft_ui_egui::{Services, state::MadeLayer, transform_tool};
use serde_json::json;

fn image_bytes(name: &str) -> Vec<u8> {
    let mut session = photocraft_engine::Session::new();
    session
        .execute(
            "file.new",
            json!({"width": 20, "height": 12, "background": "white"}),
        )
        .unwrap();
    photocraft_io::export(&session.active().unwrap().doc, name, &Default::default())
        .unwrap()
        .bytes
}

fn app(with_document: bool) -> (PhotocraftApp, egui::Context) {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut app = PhotocraftApp::new(
        photocraft_engine::Session::new(),
        Services {
            import: Some(Box::new(|name, bytes, depth| {
                photocraft_io::import_with_svg_group_depth(name, bytes, depth)
                    .map(|r| (r.document, r.warnings))
                    .map_err(|e| e.to_string())
            })),
            ..Default::default()
        },
    );
    if with_document {
        app.run(
            "file.new",
            json!({"width": 100, "height": 80, "background": "white"}),
        )
        .unwrap();
    }
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    (app, ctx)
}

fn frame(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280., 900.),
        )),
        ..Default::default()
    };
    super::super::editor_frame(ctx, app, raw).drop_without_applying_deltas();
}

fn canvas_point(app: &PhotocraftApp, ctx: &egui::Context) -> (f32, f32) {
    let center = app.last_canvas_rect.center();
    let scale = ctx.pixels_per_point();
    (center.x * scale, center.y * scale)
}

#[test]
fn canvas_drop_places_a_layer_with_transform_and_one_step_undo() {
    let (mut app, ctx) = app(true);
    let (x, y) = canvas_point(&app, &ctx);
    let before = app.session.active().unwrap().doc.layers.len();
    let id = app.session.active().unwrap().doc.id;
    let preview: serde_json::Value =
        serde_json::from_str(&feedback(&app, &ctx, x, y, 23).unwrap()).unwrap();
    assert_eq!(preview["mode"], "place");
    assert_eq!(preview["sequence"], 23);
    receive(
        &mut app,
        &ctx,
        "inserted.png".into(),
        image_bytes("p.png"),
        x,
        y,
    )
    .unwrap();
    frame(&mut app, &ctx);
    assert_eq!(app.session.documents().len(), 1);
    let state = app.session.active().unwrap();
    assert_eq!(state.doc.id, id);
    assert_eq!(state.doc.layers.len(), before + 1);
    assert_eq!(state.doc.layers.last().unwrap().name, "inserted");
    assert!(state.is_dirty());
    assert_eq!(
        app.ui.transform.as_ref().unwrap().made,
        Some(MadeLayer::Place)
    );
    transform_tool::commit(&mut app);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.layers.len(), before);
    app.run("edit.redo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.layers.len(), before + 1);
}

#[test]
fn other_positions_open_a_document_even_when_a_canvas_exists() {
    let (mut app, ctx) = app(true);
    let (x, y) = (5., 5.);
    let preview: serde_json::Value =
        serde_json::from_str(&feedback(&app, &ctx, x, y, 1).unwrap()).unwrap();
    assert_eq!(preview["mode"], "open");
    let original_layers = app.session.active().unwrap().doc.layers.len();
    receive(
        &mut app,
        &ctx,
        "/temporary/source/picture.png".into(),
        image_bytes("p.png"),
        x,
        y,
    )
    .unwrap();
    frame(&mut app, &ctx);
    assert_eq!(app.session.documents().len(), 2);
    assert_eq!(app.session.documents()[0].doc.layers.len(), original_layers);
    assert_eq!(app.session.active().unwrap().doc.name, "picture.png");
    assert!(
        app.session.active().unwrap().path.is_none(),
        "temporary paths cannot become a Save target"
    );
}

#[test]
fn no_document_drop_opens_the_image_and_psd_place_stays_one_layer() {
    let (mut app, ctx) = app(false);
    let x = 600.;
    let y = 400.;
    assert!(feedback(&app, &ctx, x, y, 1).unwrap().contains("open"));
    receive(
        &mut app,
        &ctx,
        "photo.png".into(),
        image_bytes("p.png"),
        x,
        y,
    )
    .unwrap();
    frame(&mut app, &ctx);
    let before = app.session.active().unwrap().doc.layers.len();
    let (x, y) = canvas_point(&app, &ctx);
    receive(
        &mut app,
        &ctx,
        "layered.psd".into(),
        image_bytes("p.psd"),
        x,
        y,
    )
    .unwrap();
    frame(&mut app, &ctx);
    assert_eq!(app.session.documents().len(), 1);
    assert_eq!(app.session.active().unwrap().doc.layers.len(), before + 1);
    transform_tool::cancel(&mut app);
    assert_eq!(
        app.session.active().unwrap().doc.layers.len(),
        before,
        "Escape removes the pending place"
    );
}

#[test]
fn multiple_canvas_drops_wait_for_the_previous_transform() {
    let (mut app, ctx) = app(true);
    let (x, y) = canvas_point(&app, &ctx);
    let before = app.session.active().unwrap().doc.layers.len();
    for name in ["first.png", "second.png"] {
        receive(&mut app, &ctx, name.into(), image_bytes("p.png"), x, y).unwrap();
    }
    frame(&mut app, &ctx);
    assert_eq!(app.session.active().unwrap().doc.layers.len(), before + 1);
    transform_tool::commit(&mut app);
    frame(&mut app, &ctx);
    assert_eq!(app.session.active().unwrap().doc.layers.len(), before + 2);
    assert_eq!(
        app.ui.transform.as_ref().unwrap().made,
        Some(MadeLayer::Place)
    );
    assert_eq!(app.session.documents().len(), 1);
}

#[test]
fn physical_coordinates_match_feedback_and_drop_at_different_scales() {
    let (mut app, ctx) = app(true);
    ctx.set_pixels_per_point(2.);
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    let (x, y) = canvas_point(&app, &ctx);
    assert_eq!(point(&ctx, x, y).unwrap(), app.last_canvas_rect.center());
    assert!(feedback(&app, &ctx, x, y, 2).unwrap().contains("place"));
    receive(
        &mut app,
        &ctx,
        "scaled.png".into(),
        image_bytes("p.png"),
        x,
        y,
    )
    .unwrap();
    frame(&mut app, &ctx);
    assert_eq!(app.session.documents().len(), 1);
    assert!(app.ui.transform.is_some());
}

#[test]
fn invalid_coordinates_or_bytes_cannot_mutate_a_document() {
    let (mut app, ctx) = app(true);
    let before = app.session.active().unwrap().doc.layers.len();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(receive(&mut app, &ctx, "p.png".into(), vec![1], value, 50.).is_err());
        assert!(feedback(&app, &ctx, 50., value, 1).is_err());
    }
    assert!(receive(&mut app, &ctx, "p.png".into(), Vec::new(), 50., 50.).is_err());
    let (x, y) = canvas_point(&app, &ctx);
    receive(
        &mut app,
        &ctx,
        "bad.png".into(),
        b"not an image".to_vec(),
        x,
        y,
    )
    .unwrap();
    frame(&mut app, &ctx);
    assert_eq!(app.session.documents().len(), 1);
    assert_eq!(app.session.active().unwrap().doc.layers.len(), before);
    assert!(app.ui.transform.is_none());
    assert!(app.ui.notices.iter().any(
        |n| n.title.contains("bad.png") || n.lines.iter().any(|line| line.contains("bad.png"))
    ));
}
