#[test]
fn hardware_shortcuts_use_current_input_once_and_keep_modifiers() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::Tool};
    use std::sync::{Arc, Mutex};
    let writes = Arc::new(Mutex::new(Vec::new()));
    let seen = writes.clone();
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(
        photocraft_engine::Session::new(),
        Services {
            default_save: Some(Box::new(|_| Ok("/local/keyboard.psd".into()))),
            export: Some(Box::new(|_, _, _| Ok((vec![1], Vec::new())))),
            write: Some(Box::new(move |name, _| {
                seen.lock().unwrap().push(name.to_owned());
                Ok(())
            })),
            ..Default::default()
        },
    );
    app.run("file.new", serde_json::json!({"width": 16, "height": 16}))
        .unwrap();
    app.run("layer.new.layer", serde_json::json!({})).unwrap();
    let mut input = super::input::State::default();
    let mut events = Vec::new();
    let frame =
        |app: &mut PhotocraftApp, _input: &super::input::State, events: &mut Vec<egui::Event>| {
            let mut raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 800.),
                )),
                events: std::mem::take(events),
                ..Default::default()
            };
            app.host_input(&ctx, &mut raw);
            super::editor_frame(&ctx, app, raw).drop_without_applying_deltas();
        };
    for _ in 0..3 {
        frame(&mut app, &input, &mut events);
    }
    app.ui.tool = Tool::Brush;
    input.push(
        serde_json::from_str(r#"{"kind":"key","code":2021,"text":"e"}"#).unwrap(),
        1.,
        &mut events,
    );
    frame(&mut app, &input, &mut events);
    assert_eq!(app.ui.tool, Tool::Eraser);
    input.push(
        serde_json::from_str(r#"{"kind":"key","code":2021,"action":1}"#).unwrap(),
        1.,
        &mut events,
    );
    input.push(
        serde_json::from_str(r#"{"kind":"key","code":2035,"ctrl":true}"#).unwrap(),
        1.,
        &mut events,
    );
    frame(&mut app, &input, &mut events);
    assert_eq!(writes.lock().unwrap().as_slice(), &["/local/keyboard.psd"]);
    assert!(!app.session.active().unwrap().is_dirty());
    frame(&mut app, &input, &mut events);
    assert!(ctx.input(|i| i.modifiers.ctrl));
    assert_eq!(writes.lock().unwrap().len(), 1);
    input.push(
        serde_json::from_str(r#"{"kind":"key","code":2035,"ctrl":true,"action":1}"#).unwrap(),
        1.,
        &mut events,
    );
    input.push(
        serde_json::from_str(r#"{"kind":"key","code":2072,"action":1}"#).unwrap(),
        1.,
        &mut events,
    );
    frame(&mut app, &input, &mut events);
    assert!(!ctx.input(|i| i.modifiers.ctrl));
}
#[test]
fn direct_egui_close_is_returned_to_unsaved_guard_before_termination() {
    use photocraft_ui_egui::{PhotocraftApp, Services};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width": 16, "height": 16}))
        .unwrap();
    app.run("layer.new.layer", serde_json::json!({})).unwrap();
    assert_eq!(
        super::close_action(false, &[egui::ViewportCommand::Close]),
        super::CloseAction::Request
    );
    let mut raw = egui::RawInput::default();
    raw.viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = super::editor_frame(&ctx, &mut app, raw);
    let commands = output
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .unwrap()
        .commands
        .clone();
    output.textures_delta.clear();
    assert_eq!(
        super::close_action(true, &commands),
        super::CloseAction::Cancel
    );
    assert!(app.session.active().unwrap().is_dirty());
    assert_eq!(super::close_action(true, &[]), super::CloseAction::Confirm);
}
#[test]
fn custom_host_exposes_font_definitions_before_platform_registration() {
    let ctx = egui::Context::default();
    photocraft_ui_egui::PhotocraftApp::setup_context(&ctx, Default::default());
    let mut textures = super::initialize_font_context(&ctx);
    textures.clear();
    assert!(ctx.fonts(|fonts| !fonts.definitions().font_data.is_empty()));
}
