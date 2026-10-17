use super::*;
use egui::{ViewportClass, ViewportCommand};

const ROOT_SIZE: egui::Vec2 = egui::vec2(1000., 800.);

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx
}

fn registry(scale: f32) -> Rc<RefCell<Registry>> {
    let registry = Rc::new(RefCell::new(Registry::new(scale, Default::default())));
    install(registry.clone());
    registry
}

fn doc_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("docwin-test")
}

#[derive(Default)]
struct Seen {
    calls: u32,
    screen: Option<egui::Vec2>,
    close_requested: bool,
    primary_down: bool,
    class: Option<ViewportClass>,
}

/// One root frame in which the app shows the child viewport iff `show`.
fn frame(
    ctx: &egui::Context,
    registry: &Rc<RefCell<Registry>>,
    show: bool,
    title: &str,
    seen: &mut Seen,
) -> (Finished, egui::FullOutput) {
    registry.borrow_mut().begin_frame(0.0, None, 1.0);
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, ROOT_SIZE)),
        ..Default::default()
    };
    let mut output = ctx.run_ui(raw, |ui| {
        if show {
            ui.ctx().show_viewport_immediate(
                doc_id(),
                ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([400., 300.]),
                |ui, class| {
                    seen.calls += 1;
                    seen.class = Some(class);
                    seen.screen = Some(ui.ctx().content_rect().size());
                    seen.close_requested = ui.ctx().input(|i| i.viewport().close_requested());
                    seen.primary_down = ui.ctx().input(|i| i.pointer.primary_down());
                },
            );
        }
    });
    let mut finished = registry.borrow_mut().finish_frame(&output.viewport_output);
    finished.textures.clear();
    output.textures_delta.clear();
    (finished, output)
}

#[test]
fn opening_a_viewport_requests_one_native_window_and_always_runs_the_callback() {
    let ctx = context();
    let registry = registry(2.);
    let mut seen = Seen::default();
    let (finished, _) = frame(&ctx, &registry, true, "doc", &mut seen);
    // egui panics if the immediate callback is skipped, even before the surface exists.
    assert_eq!(seen.calls, 1);
    assert!(seen.class == Some(ViewportClass::Immediate));
    assert_eq!(finished.frames.len(), 1);
    assert_eq!(
        registry.borrow_mut().take_outbox(),
        vec![Command::Open {
            key: 1,
            title: "doc".into(),
            width: 800,
            height: 600,
            x: None,
            y: None,
        }]
    );
    // The window is requested once, not on every frame.
    let (finished, _) = frame(&ctx, &registry, true, "doc", &mut seen);
    assert_eq!(finished.frames.len(), 1);
    assert!(registry.borrow_mut().take_outbox().is_empty());
}

#[test]
fn a_viewport_is_sized_from_its_own_surface() {
    let ctx = context();
    let registry = registry(2.);
    let mut seen = Seen::default();
    frame(&ctx, &registry, true, "doc", &mut seen);
    registry.borrow_mut().attach(1, 1000, 500);
    assert!(registry.borrow().attached(1));
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert_eq!(seen.screen, Some(egui::vec2(500., 250.)));
    registry.borrow_mut().resize(1, 1200, 600);
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert_eq!(seen.screen, Some(egui::vec2(600., 300.)));
}

#[test]
fn input_reaches_only_the_window_it_was_sent_to() {
    let ctx = context();
    let registry = registry(1.);
    let mut seen = Seen::default();
    frame(&ctx, &registry, true, "doc", &mut seen);
    registry.borrow_mut().attach(1, 400, 300);
    let press: input::Input = serde_json::from_str(
        r#"{"kind":"pointer","source":"mouse","action":0,"x":50,"y":60}"#,
    )
    .unwrap();
    registry.borrow_mut().push_input(1, press, 1.);
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert!(seen.primary_down, "the sub-window saw its press");
    // The root viewport's own input is untouched by the child's events.
    assert!(!ctx.input(|i| i.pointer.primary_down()));
    // Unknown windows ignore input.
    registry.borrow_mut().push_input(
        9,
        serde_json::from_str(r#"{"kind":"blur"}"#).unwrap(),
        1.,
    );
}

#[test]
fn system_close_becomes_a_viewport_close_then_the_window_is_destroyed() {
    let ctx = context();
    let registry = registry(1.);
    let mut seen = Seen::default();
    frame(&ctx, &registry, true, "doc", &mut seen);
    registry.borrow_mut().take_outbox();
    registry.borrow_mut().request_close(1);
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert!(seen.close_requested, "extra_windows sees close_requested()");
    // Close is a one-shot event.
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert!(!seen.close_requested);
    // The app reacts by no longer showing the window.
    let (finished, _) = frame(&ctx, &registry, false, "doc", &mut seen);
    assert_eq!(finished.closed, vec![1]);
    assert!(finished.frames.is_empty());
    assert_eq!(registry.borrow_mut().take_outbox(), vec![Command::Close { key: 1 }]);
    assert!(!registry.borrow().knows(1));
    // Showing it again opens a fresh native window with a new key.
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert!(matches!(
        registry.borrow_mut().take_outbox().as_slice(),
        [Command::Open { key: 2, .. }]
    ));
}

#[test]
fn losing_the_surface_keeps_the_window_registered() {
    let ctx = context();
    let registry = registry(1.);
    let mut seen = Seen::default();
    frame(&ctx, &registry, true, "doc", &mut seen);
    registry.borrow_mut().attach(1, 400, 300);
    registry.borrow_mut().take_outbox();
    // App backgrounded: surface destroyed, but the user did not close the window.
    registry.borrow_mut().detach(1);
    assert!(!registry.borrow().attached(1));
    let (finished, _) = frame(&ctx, &registry, true, "doc", &mut seen);
    assert!(finished.closed.is_empty());
    assert!(!seen.close_requested);
    assert!(registry.borrow_mut().take_outbox().is_empty());
    registry.borrow_mut().attach(1, 640, 480);
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert_eq!(seen.screen, Some(egui::vec2(640., 480.)));
}

#[test]
fn title_and_size_commands_follow_the_app() {
    let ctx = context();
    let registry = registry(2.);
    let mut seen = Seen::default();
    frame(&ctx, &registry, true, "doc", &mut seen);
    registry.borrow_mut().take_outbox();
    frame(&ctx, &registry, true, "renamed", &mut seen);
    assert_eq!(
        registry.borrow_mut().take_outbox(),
        vec![Command::Title {
            key: 1,
            title: "renamed".into()
        }]
    );
    ctx.send_viewport_cmd_to(doc_id(), ViewportCommand::InnerSize(egui::vec2(300., 200.)));
    frame(&ctx, &registry, true, "renamed", &mut seen);
    assert_eq!(
        registry.borrow_mut().take_outbox(),
        vec![Command::Resize {
            key: 1,
            width: 600,
            height: 400
        }]
    );
}

#[test]
fn embedded_viewports_never_touch_the_registry() {
    let ctx = egui::Context::default(); // embed_viewports defaults to true
    let registry = registry(1.);
    let mut seen = Seen::default();
    frame(&ctx, &registry, true, "doc", &mut seen);
    assert!(seen.class == Some(ViewportClass::EmbeddedWindow));
    assert!(registry.borrow().is_empty());
    assert!(registry.borrow_mut().take_outbox().is_empty());
}

#[test]
fn window_commands_are_json_for_arkts() {
    let open = Command::Open {
        key: 3,
        title: "Untitled — window 3".into(),
        width: 800,
        height: 600,
        x: Some(10),
        y: None,
    };
    let value: serde_json::Value = serde_json::from_str(&open.json()).unwrap();
    assert_eq!(value["op"], "open");
    assert_eq!(value["win"], 3);
    assert_eq!(value["title"], "Untitled — window 3");
    assert_eq!(value["x"], 10);
    assert!(value["y"].is_null());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&Command::Close { key: 3 }.json()).unwrap(),
        serde_json::json!({"op": "close", "win": 3})
    );
}

/// The real upstream path: `canvas::extra_windows` opts into native windows only while it
/// runs, so other viewports (Camera Raw) keep today's in-app behaviour.
#[test]
fn floating_document_windows_become_native_windows_without_embedding_the_rest() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::DocWindow};
    let ctx = egui::Context::default();
    assert!(ctx.embed_viewports());
    let registry = registry(1.);
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.native_doc_windows = true;
    app.run("file.new", serde_json::json!({"width": 16, "height": 16}))
        .unwrap();
    app.ui.windows.push(DocWindow {
        id: 7,
        document: 0,
        view: Default::default(),
        open: true,
        position: Some([20., 30.]),
    });
    let run = |app: &mut PhotocraftApp| {
        registry.borrow_mut().begin_frame(0.0, None, 1.0);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, ROOT_SIZE)),
            ..Default::default()
        };
        let mut output = super::super::editor_frame(&ctx, app, raw);
        let mut finished = registry.borrow_mut().finish_frame(&output.viewport_output);
        finished.textures.clear();
        output.textures_delta.clear();
        finished.frames.len()
    };
    // The editor needs a few frames before it draws its windows (fonts, layout).
    for _ in 0..3 {
        run(&mut app);
    }
    assert_eq!(run(&mut app), 1, "one child pass for the floating window");
    assert!(ctx.embed_viewports(), "embedding restored for everything else");
    match registry.borrow_mut().take_outbox().as_slice() {
        [Command::Open { key: 1, title, x: Some(20), y: Some(30), .. }] => {
            assert!(title.ends_with("window 7"), "{title}");
        }
        other => panic!("unexpected window commands: {other:?}"),
    }
    // The system close button asks the app to close the window; it then goes away.
    registry.borrow_mut().request_close(1);
    run(&mut app);
    assert!(app.ui.windows.is_empty(), "extra_windows honoured the close request");
    run(&mut app);
    assert_eq!(registry.borrow_mut().take_outbox(), vec![Command::Close { key: 1 }]);
}

#[test]
fn without_native_support_floating_windows_stay_embedded() {
    use photocraft_ui_egui::{PhotocraftApp, Services, state::DocWindow};
    let ctx = egui::Context::default();
    let registry = registry(1.);
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", serde_json::json!({"width": 16, "height": 16}))
        .unwrap();
    app.ui.windows.push(DocWindow {
        id: 1,
        document: 0,
        view: Default::default(),
        open: true,
        position: None,
    });
    registry.borrow_mut().begin_frame(0.0, None, 1.0);
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, ROOT_SIZE)),
        ..Default::default()
    };
    super::super::editor_frame(&ctx, &mut app, raw).drop_without_applying_deltas();
    assert!(registry.borrow().is_empty());
}
