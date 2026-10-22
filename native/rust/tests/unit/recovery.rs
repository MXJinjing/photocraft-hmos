use super::*;

#[test]
fn failed_gpu_halts_drawing_until_restart() {
    let mut recovery = Recovery::default();
    let before = Instant::now();
    recovery.failed();
    assert!(recovery.halted);
    assert_eq!(recovery.frame_deadline(before), before);
}

#[test]
fn failed_state_is_latched_until_restart() {
    let mut recovery = Recovery::default();
    recovery.failed();
    assert!(recovery.halted);
    assert!(recovery.halted);
}

#[test]
fn stopping_gpu_preserves_dirty_document_and_undo_history() {
    let mut app = photocraft_ui_egui::PhotocraftApp::new(
        photocraft_engine::Session::new(),
        photocraft_ui_egui::Services::default(),
    );
    app.run("file.new", serde_json::json!({"width": 16, "height": 16}))
        .unwrap();
    let initial_layers = app.session.active().unwrap().doc.layers.len();
    app.run("layer.new.layer", serde_json::json!({})).unwrap();
    let id = app.session.active().unwrap().doc.id;
    let revision = app.session.active().unwrap().revision;
    stop_gpu(&mut app, "injected device loss");
    assert_eq!(app.session.active().unwrap().doc.id, id);
    assert_eq!(app.session.active().unwrap().revision, revision);
    assert!(app.session.active().unwrap().is_dirty());
    assert!(!app.gpu_active());
    app.run("edit.undo", serde_json::json!({})).unwrap();
    assert_eq!(
        app.session.active().unwrap().doc.layers.len(),
        initial_layers
    );
}

#[test]
fn panic_in_ui_pass_is_closed_before_stopping_gpu() {
    let ctx = egui::Context::default();
    let raw = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(200.0, 100.0),
        )),
        ..Default::default()
    };
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.run_ui(raw(), |ui| {
            ui.label("Before panic: 字体");
            panic!("injected first-use compositor panic");
        })
        .drop_without_applying_deltas();
    }));
    assert!(failure.is_err());
    finish_interrupted_pass(&ctx).unwrap();
    let mut output = ctx.run_ui(raw(), |ui| {
        ui.label("After recovery");
    });
    output.textures_delta.clear();
    assert!(!output.shapes.is_empty());
}
