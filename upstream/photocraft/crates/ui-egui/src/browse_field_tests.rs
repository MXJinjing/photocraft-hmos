//! #7942 / #7531: path fields of command dialogs get a Browse… / Choose… button
//! (Artboards to Files' folder, Install Plug-in…'s .wasm file) that fills the field
//! from a file dialog. On sandboxed hosts (`path_pickers_only`) the label is Choose…
//! and the host pickers (`pick_folder` / `pick_open`) answer.
use crate::PhotocraftApp;
#[cfg(not(feature = "harmonyos-ui"))]
use crate::file_dialog::{FileDialogAnswer, FileDialogRequest};
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

fn browse_label() -> &'static str {
    if cfg!(feature = "harmonyos-ui") {
        "Choose…"
    } else {
        "Browse…"
    }
}

fn field(h: &Harness<'_, PhotocraftApp>, key: &str) -> Option<Value> {
    h.state().ui.dialogs.last().and_then(|d| d.fields.get(key).cloned())
}

#[cfg(not(feature = "harmonyos-ui"))]
fn harness(answers: Vec<Option<FileDialogAnswer>>) -> (Harness<'static, PhotocraftApp>, Rc<RefCell<Vec<FileDialogRequest>>>) {
    let (show, asked) = crate::file_dialog::fake(answers);
    let h = Harness::builder().with_size(vec2(1440.0, 900.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services { file_dialog: Some(show), ..Default::default() })
    });
    (h, asked)
}

#[cfg(feature = "harmonyos-ui")]
struct SandboxAnswers {
    folder: RefCell<Vec<Option<String>>>,
    open: RefCell<Vec<Option<(String, Vec<u8>)>>>,
    folder_asked: RefCell<Vec<bool>>,
}

#[cfg(feature = "harmonyos-ui")]
fn sandbox_harness(folders: Vec<Option<String>>, opens: Vec<Option<(String, Vec<u8>)>>) -> (Harness<'static, PhotocraftApp>, Rc<SandboxAnswers>) {
    let answers = Rc::new(SandboxAnswers {
        folder: RefCell::new(folders),
        open: RefCell::new(opens),
        folder_asked: RefCell::new(Vec::new()),
    });
    let a_folder = answers.clone();
    let a_open = answers.clone();
    let sandbox = std::env::temp_dir().join(format!("photocraft-browse-field-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&sandbox);
    let h = Harness::builder().with_size(vec2(1440.0, 900.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(
            photocraft_engine::Session::new(),
            crate::Services {
                path_pickers_only: true,
                sandbox_dir: Some(sandbox.to_string_lossy().into_owned()),
                pick_folder: Some(Box::new(move |output| {
                    a_folder.folder_asked.borrow_mut().push(output);
                    a_folder.folder.borrow_mut().pop().flatten().map(Ok)
                })),
                pick_open: Some(Box::new(move || {
                    a_open.open.borrow_mut().pop().flatten().map(|(name, bytes)| (name, Ok(bytes)))
                })),
                ..Default::default()
            },
        )
    });
    (h, answers)
}

#[test]
fn artboards_to_files_folder_is_chosen_with_browse() {
    #[cfg(not(feature = "harmonyos-ui"))]
    {
        let (mut h, asked) = harness(vec![Some(FileDialogAnswer::FolderTo("/tmp/exports".into()))]);
        h.state_mut().run("file.new", json!({"width": 64, "height": 64})).unwrap();
        h.state_mut().sync_views();
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "file.export.artboardsToFiles", json!({})).unwrap();
        h.run_steps(4);
        assert!(field(&h, "dir").is_some(), "the dialog has a Dir field");
        h.get_by_label(browse_label()).click();
        h.run_steps(4);
        assert!(matches!(asked.borrow().as_slice(), [FileDialogRequest::Folder { .. }]));
        assert_eq!(field(&h, "dir"), Some(json!("/tmp/exports")));
        assert!(field(&h, crate::dialogs::BROWSE_FIELD).is_none(), "the request is consumed");
    }
    #[cfg(feature = "harmonyos-ui")]
    {
        // pick_folder pops from the end.
        let (mut h, asked) = sandbox_harness(vec![Some("/tmp/exports".into())], vec![]);
        h.state_mut().run("file.new", json!({"width": 64, "height": 64})).unwrap();
        h.state_mut().sync_views();
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "file.export.artboardsToFiles", json!({})).unwrap();
        h.run_steps(4);
        assert!(field(&h, "dir").is_some(), "the dialog has a Dir field");
        h.get_by_label(browse_label()).click();
        h.run_steps(4);
        assert_eq!(*asked.folder_asked.borrow(), [true], "output folder picker");
        assert_eq!(field(&h, "dir"), Some(json!("/tmp/exports")));
    }
}

#[test]
fn cancelling_browse_keeps_the_typed_folder() {
    #[cfg(not(feature = "harmonyos-ui"))]
    {
        let (mut h, _) = harness(vec![None]);
        h.state_mut().run("file.new", json!({"width": 64, "height": 64})).unwrap();
        h.state_mut().sync_views();
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "file.export.layersToFiles", json!({})).unwrap();
        h.run_steps(4);
        let before = field(&h, "dir");
        h.get_by_label(browse_label()).click();
        h.run_steps(4);
        assert_eq!(field(&h, "dir"), before);
    }
    #[cfg(feature = "harmonyos-ui")]
    {
        let (mut h, _) = sandbox_harness(vec![None], vec![]);
        h.state_mut().run("file.new", json!({"width": 64, "height": 64})).unwrap();
        h.state_mut().sync_views();
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "file.export.layersToFiles", json!({})).unwrap();
        h.run_steps(4);
        let before = field(&h, "dir");
        h.get_by_label(browse_label()).click();
        h.run_steps(4);
        assert_eq!(field(&h, "dir"), before);
    }
}

#[test]
fn install_plugin_path_is_chosen_with_browse() {
    #[cfg(not(feature = "harmonyos-ui"))]
    {
        let (mut h, asked) = harness(vec![Some(FileDialogAnswer::Paths(vec!["/tmp/blur.wasm".into()]))]);
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "plugin.install", json!({})).unwrap();
        h.run_steps(4);
        h.get_by_label(browse_label()).click();
        h.run_steps(4);
        match asked.borrow().as_slice() {
            [FileDialogRequest::Open { multiple: false, extensions: Some(e), .. }] => assert_eq!(e, &["wasm".to_string()]),
            other => panic!("unexpected requests {other:?}"),
        }
        assert_eq!(field(&h, "path"), Some(json!("/tmp/blur.wasm")));
    }
    #[cfg(feature = "harmonyos-ui")]
    {
        let (mut h, _) = sandbox_harness(vec![], vec![Some(("/tmp/blur.wasm".into(), b"\0asm".to_vec()))]);
        let ctx = h.ctx.clone();
        crate::menus::invoke(h.state_mut(), &ctx, "plugin.install", json!({})).unwrap();
        h.run_steps(4);
        h.get_by_label(browse_label()).click();
        h.run_steps(4);
        let path = field(&h, "path").and_then(|v| v.as_str().map(str::to_owned)).expect("path filled");
        assert!(path.ends_with("blur.wasm") || path.ends_with("Blur.wasm"), "sandbox copy: {path}");
    }
}
