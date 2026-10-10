use super::*;
fn frame(ctx: &egui::Context, text: &mut String, events: Vec<Event>) -> egui::PlatformOutput {
    frame_editor(ctx, text, events, egui::Id::new("editor"))
}
fn frame_editor(
    ctx: &egui::Context,
    text: &mut String,
    events: Vec<Event>,
    id: egui::Id,
) -> egui::PlatformOutput {
    let raw = egui::RawInput {
        events,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(600., 400.),
        )),
        ..Default::default()
    };
    let _ = ctx.run_logic(&raw, |_| {});
    let mut output = ctx.run_ui(raw, |ui| {
        ui.add(egui::TextEdit::singleline(text).id(id));
    });
    output.textures_delta.clear();
    output.platform_output
}
fn command(
    sync: &mut ImeSync,
    ctx: &egui::Context,
    kind: &str,
    text: &str,
    count: usize,
) -> Vec<Event> {
    let mut events = vec![];
    sync.input(
        ctx,
        None,
        crate::input::Input {
            kind: kind.into(),
            text: text.into(),
            count,
            focus: sync.last.as_ref().unwrap().focus.clone(),
            ..Default::default()
        },
        &mut events,
    );
    events
}
#[test]
fn help_search_automatic_focus_opens_keyboard_once_and_accepts_ime() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("help-menu-search").with("field");
    let mut text = String::new();
    let mut sync = ImeSync::default();
    frame_editor(&ctx, &mut text, vec![], id);
    for _ in 0..2 {
        ctx.memory_mut(|m| m.request_focus(id));
        let out = frame_editor(&ctx, &mut text, vec![], id);
        sync.update(&ctx, &out, None);
        assert_eq!(sync.last.as_ref().unwrap().show_request, 1);
        let events = command(&mut sync, &ctx, "ime_commit", "你", 0);
        let out = frame_editor(&ctx, &mut text, events, id);
        sync.update(&ctx, &out, None);
        assert_eq!(
            sync.last.as_ref().unwrap().show_request,
            1,
            "typing must not request the keyboard again"
        );
        ctx.memory_mut(|m| m.surrender_focus(id));
        let out = frame_editor(&ctx, &mut text, vec![], id);
        sync.update(&ctx, &out, None);
        assert!(!sync.last.as_ref().unwrap().active);
    }
    assert_eq!(text, "你你");
}

#[test]
fn real_text_edit_accepts_composition_commit_unicode_delete_and_rejects_stale_focus() {
    let ctx = egui::Context::default();
    let mut text = String::new();
    let mut sync = ImeSync::default();
    frame(&ctx, &mut text, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("editor")));
    let out = frame(&ctx, &mut text, vec![]);
    assert!(out.ime.is_some());
    assert!(sync.update(&ctx, &out, None).is_some());
    assert_eq!(
        sync.last.as_ref().unwrap().show_request,
        0,
        "automatic focus must not open the keyboard"
    );
    sync.pointer_input(&[Event::PointerButton {
        pos: ctx
            .read_response(egui::Id::new("editor"))
            .unwrap()
            .rect
            .right_center()
            - egui::vec2(2., 0.),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    sync.update(&ctx, &out, None);
    assert_eq!(sync.last.as_ref().unwrap().show_request, 1);
    let old_focus = sync.last.as_ref().unwrap().focus.clone();
    for (kind, value, expected) in [
        ("ime_preedit", "ni", "ni"),
        ("ime_preedit", "你好", "你好"),
        ("ime_commit", "你好😀", "你好😀"),
    ] {
        let events = command(&mut sync, &ctx, kind, value, 0);
        let out = frame(&ctx, &mut text, events);
        sync.update(&ctx, &out, None);
        assert_eq!(text, expected);
    }
    assert_eq!(
        sync.last.as_ref().unwrap().end,
        4,
        "HarmonyOS selection uses UTF-16 offsets"
    );
    let events = command(&mut sync, &ctx, "ime_delete_left", "", 2);
    let out = frame(&ctx, &mut text, events);
    sync.update(&ctx, &out, None);
    assert_eq!(text, "你好");
    sync.suspend();
    let out = frame(&ctx, &mut text, vec![]);
    sync.update(&ctx, &out, None);
    assert_ne!(sync.last.as_ref().unwrap().focus, old_focus);
    let mut events = vec![];
    sync.input(
        &ctx,
        None,
        crate::input::Input {
            kind: "ime_commit".into(),
            focus: old_focus,
            text: "stale".into(),
            ..Default::default()
        },
        &mut events,
    );
    assert!(events.is_empty());
    ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("editor")));
    let out = frame(&ctx, &mut text, vec![]);
    sync.update(&ctx, &out, None);
    assert!(!sync.last.as_ref().unwrap().active);
}

#[test]
fn canvas_type_session_opens_ime_and_accepts_composition_and_unicode_deletion() {
    use photocraft_engine::doc::{LayerContent, LayerId};
    use photocraft_ui_egui::{PhotocraftApp, Services, state::TextEdit};
    let ctx = egui::Context::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.ui.tool = photocraft_ui_egui::state::Tool::Type;
    app.run("file.new", serde_json::json!({"width":128,"height":128}))
        .unwrap();
    let layer = app
        .run(
            "type.create",
            serde_json::json!({"text":"", "size":24,"x":10,"y":30}),
        )
        .unwrap()["layer"]
        .as_u64()
        .unwrap();
    app.ui.text_edit = Some(TextEdit {
        layer,
        caret: 0,
        anchor: 0,
        session: "test-type".into(),
        created: false,
        dragging: false,
        resize: None,
        preedit: None,
    });
    let rect = egui::Rect::from_min_size(egui::pos2(20., 30.), egui::vec2(1., 24.));
    let mut output = egui::PlatformOutput::default();
    output.ime = Some(egui::output::IMEOutput {
        rect,
        cursor_rect: rect,
        purpose: egui::IMEPurpose::Normal,
        should_interrupt_composition: false,
    });
    let mut sync = ImeSync::default();
    sync.update(&ctx, &output, Some(&app));
    assert_eq!(sync.last.as_ref().unwrap().show_request, 1);
    app.last_canvas_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100., 40.));
    let before = app.ui.views[app.session.active_index().unwrap()].center[1];
    assert!(reveal_canvas_caret(&mut app, &output));
    assert!(app.ui.views[app.session.active_index().unwrap()].center[1] > before);
    let visible = egui::Rect::from_min_size(egui::pos2(20., 10.), egui::vec2(1., 10.));
    let mut inside = output.clone();
    inside.ime.as_mut().unwrap().rect = visible;
    inside.ime.as_mut().unwrap().cursor_rect = visible;
    assert!(!reveal_canvas_caret(&mut app, &inside));
    for (kind, text, count, expected) in [
        ("ime_preedit", "ni", 0, "ni"),
        ("ime_commit", "你好😀", 0, "你好😀"),
        ("ime_delete_left", "", 2, "你好"),
    ] {
        let mut events = vec![];
        sync.input(
            &ctx,
            Some(&mut app),
            crate::input::Input {
                kind: kind.into(),
                text: text.into(),
                count,
                focus: sync.last.as_ref().unwrap().focus.clone(),
                ..Default::default()
            },
            &mut events,
        );
        let raw = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut frame = ctx.run_ui(raw, |ui| {
            photocraft_ui_egui::type_tool::handle_keys(&mut app, ui.ctx());
        });
        frame.textures_delta.clear();
        let LayerContent::Text(value) = &app
            .session
            .active()
            .unwrap()
            .doc
            .layer(LayerId(layer))
            .unwrap()
            .content
        else {
            panic!("text layer")
        };
        assert_eq!(value.text, expected);
        sync.update(&ctx, &output, Some(&app));
    }
}
