use super::*;
use crate::Services;

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("photocraft-path-pick-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sandboxed(sandbox: &Path, services: Services) -> PhotocraftApp {
    PhotocraftApp::new(
        photocraft_engine::Session::new(),
        Services { path_pickers_only: true, sandbox_dir: Some(sandbox.to_string_lossy().into_owned()), ..services },
    )
}

#[test]
fn path_fields_are_known_by_command_and_key() {
    assert_eq!(kind_for("plugin.install", "path"), file(&["wasm"], Stage::Plugins));
    assert_eq!(kind_for("prefs", "plugIns.additionalPluginsFolder"), Some(PickKind::PluginFolder));
    assert_eq!(kind_for("file.automate.batch", "input"), Some(PickKind::InputFolder));
    assert_eq!(kind_for("file.automate.batch", "output"), Some(PickKind::OutputFolder));
    assert_eq!(kind_for("file.export.layersToFiles", "dir"), Some(PickKind::OutputFolder));
    assert!(matches!(kind_for("file.export.artboardsToPdf", "path"), Some(PickKind::OutputFile { .. })));
    assert!(matches!(kind_for("image.adjustments.colorLookup", "file"), Some(PickKind::File { .. })));
    // Not paths: names, prefixes, ordinary text.
    assert_eq!(kind_for("file.automate.batch", "naming"), None);
    assert_eq!(kind_for("file.export.layersToFiles", "prefix"), None);
    assert_eq!(kind_for("prefs", "interface.language"), None);
}

#[test]
fn picked_names_lose_their_folders() {
    assert_eq!(safe_name("/storage/Users/a/b.wasm"), "b.wasm");
    assert_eq!(safe_name("..\\x?.icc"), "x_.icc");
    assert_eq!(safe_name(".."), "file");
    assert_eq!(safe_name(""), "file");
}

#[test]
fn pointers_set_nested_fields_and_create_missing_keys() {
    let mut f = serde_json::from_value::<Map<String, Value>>(json!({"values": {"plugIns": {"a": 1}}, "list": [{"value": ""}]})).unwrap();
    set_pointer(&mut f, "/values/plugIns/additionalPluginsFolder", json!("/x")).unwrap();
    set_pointer(&mut f, "/list/0/value", json!("/y")).unwrap();
    set_pointer(&mut f, "/path", json!("/z")).unwrap();
    assert_eq!(f["values"]["plugIns"]["additionalPluginsFolder"], "/x");
    assert_eq!(f["values"]["plugIns"]["a"], 1);
    assert_eq!(f["list"][0]["value"], "/y");
    assert_eq!(f["path"], "/z");
    assert!(set_pointer(&mut f, "/gone/key", json!(1)).is_err());
    assert!(set_pointer(&mut f, "/list/3/value", json!(1)).is_err());
    assert_eq!(f["path"], "/z", "a failed set keeps the fields");
}

#[test]
fn the_field_is_locked_only_on_picker_only_hosts() {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut drawn = Vec::new();
    for on in [false, true] {
        ctx.run_ui(Default::default(), |ui| {
            publish(ui.ctx(), on);
            drawn.push(locked_field(ui, kind_for("plugin.install", "path"), "/path", "", 270.0));
            // Not a path field: always the caller's own field.
            drawn.push(locked_field(ui, None, "/name", "", 270.0));
        })
        .textures_delta
        .clear();
    }
    assert_eq!(drawn, [false, false, true, false]);
}

#[test]
fn a_picked_plugin_is_copied_into_the_sandbox() {
    let sandbox = temp("plugin");
    let mut app =
        sandboxed(&sandbox, Services { pick_open: Some(Box::new(|| Some(("/outside/Blur.wasm".into(), Ok(b"\0asm".to_vec()))))), ..Default::default() });
    let id = crate::filter_dialog::open(&mut app, "plugin.install").unwrap();
    start(&mut app, id, Request { pointer: "/path".into(), kind: kind_for("plugin.install", "path").unwrap() });
    let want = sandbox.join("plugins").join("Blur.wasm");
    assert_eq!(app.ui.dialog_mut(id).unwrap().fields["path"], json!(want.to_string_lossy()));
    assert_eq!(std::fs::read(&want).unwrap(), b"\0asm");
    assert!(!app.ui.status_error, "{}", app.ui.status);
}

#[test]
fn folders_come_from_the_host_picker_with_their_purpose() {
    let sandbox = temp("folder");
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = asked.clone();
    let mut app = sandboxed(
        &sandbox,
        Services {
            pick_folder: Some(Box::new(move |output| {
                log.borrow_mut().push(output);
                Some(Ok(if output { "/granted/out".into() } else { "/granted/in".into() }))
            })),
            ..Default::default()
        },
    );
    let id = app.ui.open_dialog(
        crate::state::DialogKind::Command,
        serde_json::from_value(json!({"__form": true, "input": "", "output": "", "path": "/old/Board.pdf"})).unwrap(),
    );
    start(&mut app, id, Request { pointer: "/input".into(), kind: PickKind::InputFolder });
    start(&mut app, id, Request { pointer: "/output".into(), kind: PickKind::OutputFolder });
    start(&mut app, id, Request { pointer: "/path".into(), kind: kind_for("file.export.artboardsToPdf", "path").unwrap() });
    let f = &app.ui.dialog_mut(id).unwrap().fields;
    assert_eq!(f["input"], "/granted/in");
    assert_eq!(f["output"], "/granted/out");
    assert_eq!(f["path"], json!(Path::new("/granted/out").join("Board.pdf").to_string_lossy()), "the file name is kept");
    assert_eq!(*asked.borrow(), [false, true, true]);
}

#[test]
fn a_folder_picker_error_is_reported_and_cancel_is_not() {
    let sandbox = temp("folder-error");
    let mut answers = vec![None, Some(Err("this device can't choose folders".to_string()))].into_iter();
    let mut app = sandboxed(&sandbox, Services { pick_folder: Some(Box::new(move |_| answers.next().flatten())), ..Default::default() });
    let id = app.ui.open_dialog(crate::state::DialogKind::Command, serde_json::from_value(json!({"__form": true, "input": ""})).unwrap());
    start(&mut app, id, Request { pointer: "/input".into(), kind: PickKind::InputFolder });
    assert!(!app.ui.status_error);
    start(&mut app, id, Request { pointer: "/input".into(), kind: PickKind::InputFolder });
    assert!(app.ui.status_error);
    assert_eq!(app.ui.status, "this device can't choose folders");
    assert_eq!(app.ui.dialog_mut(id).unwrap().fields["input"], "");
}

#[test]
fn a_plugin_folder_is_copied_into_the_sandbox() {
    let sandbox = temp("plugin-folder");
    let outside = temp("plugin-folder-outside");
    std::fs::write(outside.join("a.wasm"), b"a").unwrap();
    std::fs::write(outside.join("notes.txt"), b"n").unwrap();
    let from = outside.to_string_lossy().into_owned();
    let mut app = sandboxed(&sandbox, Services { pick_folder: Some(Box::new(move |_| Some(Ok(from.clone())))), ..Default::default() });
    let id =
        app.ui.open_dialog(crate::state::DialogKind::Command, serde_json::from_value(json!({"values": {"plugIns": {"additionalPluginsFolder": ""}}})).unwrap());
    start(&mut app, id, Request { pointer: "/values/plugIns/additionalPluginsFolder".into(), kind: PickKind::PluginFolder });
    let dir = sandbox.join("plugins");
    assert_eq!(app.ui.dialog_mut(id).unwrap().fields["values"]["plugIns"]["additionalPluginsFolder"], json!(dir.to_string_lossy()));
    assert!(dir.join("a.wasm").is_file());
    assert!(!dir.join("notes.txt").exists());
    // A folder without plug-ins says so.
    let empty = temp("plugin-folder-empty");
    assert!(stage_plugins(&sandbox, &empty).unwrap_err().contains("no .wasm"));
}
