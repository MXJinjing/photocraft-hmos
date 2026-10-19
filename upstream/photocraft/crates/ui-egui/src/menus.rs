//! Menu bar generated from the engine command registry plus UI-level commands.

use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::state::DialogKind;

/// Shared gate for native menu clicks and control-channel `ui.menu.invoke`.
/// Dialogs and unsaved-changes prompts block edits, but allow the four view-navigation
/// commands also usable through shortcuts. Camera Raw blocks menu commands entirely, and a pending
/// crop blocks the commands Photoshop greys during one (`crop_ui::blocks`).
pub(crate) fn modal_allows(app: &PhotocraftApp, id: &str) -> bool {
    if app.camera_raw.is_some() || crate::crop_ui::blocks(app, id) {
        return false;
    }
    (app.ui.dialogs.is_empty() && app.discard.is_none()) || crate::shortcuts::NAV_COMMANDS.contains(&id)
}

/// Top-level menus in Photoshop order.
pub const TOP_MENUS: [&str; 10] = ["File", "Edit", "Image", "Layer", "Type", "Select", "Filter", "View", "Window", "Help"];

/// UI-level commands (handled by the shell rather than the engine): id, label, menu, shortcut.
/// The table is the engine's ([`photocraft_engine::shell_commands::SHELL_COMMANDS`]), so the
/// commands and their keys exist without this UI (headless shortcut sets, lists, conflicts).
pub const UI_COMMANDS: &[(&str, &str, &[&str], Option<&str>)] = photocraft_engine::shell_commands::SHELL_COMMANDS;

/// Window › <document>: the open documents, listed at the end of the Window menu as in Photoshop.
/// The id ends in the document's index, which is its tab's position.
pub(crate) const DOCUMENT_ITEM: &str = "window.document.";

/// Photoshop's Window › <panel> ids for the panels the shell already has, as `window.toggle.*`.
pub(crate) fn panel_alias(id: &str) -> Option<&'static str> {
    Some(match id.strip_prefix("window.panel.")? {
        "layers" => "window.toggle.layers",
        "history" => "window.toggle.history",
        "properties" | "adjustments" => "window.toggle.properties",
        "color" | "swatches" => "window.toggle.color",
        "navigator" | "info" | "histogram" => "window.toggle.navigator",
        "tools" => "window.toggle.toolbar",
        "options" => "window.toggle.options",
        "brushSettings" => "window.toggle.brushSettings",
        "brushes" => "window.toggle.brushes",
        _ => return None,
    })
}

/// View › Proof Setup presets we can simulate, as `view.proofSetup` profiles.
fn proof_preset(id: &str) -> Option<&'static str> {
    Some(match id.strip_prefix("view.proofSetup.")? {
        "workingCmyk" => "working-cmyk",
        "internetStandardRgb" | "monitorRgb" => "srgb",
        _ => return None,
    })
}

/// Window › Workspace presets by id.
fn workspace_name(id: &str) -> Option<&'static str> {
    Some(match id.strip_prefix("window.workspace.")? {
        "essentials" | "resetWorkspace" => "",
        "photography" => "Photography",
        "painting" => "Painting",
        "graphicAndWeb" => "Graphic and Web",
        "pixelArt" => "Pixel Art",
        "motion" => "Motion",
        _ => return None,
    })
}

/// Run a command id from any source (menu, shortcut, palette, automation).
pub fn invoke(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: Value) -> Result<Value, String> {
    if app.automation_input
        && let Some(authorize) = app.services.automation_command.as_ref()
    {
        authorize(id, &params)?;
    }
    if crate::discard_ui::intercept(app, id, &params) {
        return Ok(Value::Null);
    }
    let r = invoke_unguarded(app, ctx, id, params)?;
    // The palette's empty-query list (UI-217-7); opening the palette isn't a command to recall.
    if id != "edit.search" {
        crate::palette::note_recent(ctx, id);
    }
    Ok(r)
}

/// [`invoke`] without the unsaved-changes prompt, for once the user has already answered it.
pub(crate) fn invoke_unguarded(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: Value) -> Result<Value, String> {
    if let Some(tool) = crate::tool_shortcuts::tool(id) {
        crate::tool_memory::select(app, tool);
        return Ok(json!({"tool": tool}));
    }
    if id == "ui.symmetryTransform" {
        return crate::symmetry_ui::begin(app).map(|_| Value::Null);
    }
    if let Some(r) = crate::tool_reset::invoke(app, id, &params) {
        return r;
    }
    // A filter on a type layer asks to rasterize it first (#4303).
    if let Some(r) = crate::rasterize_prompt::intercept_filter(app, id, &params) {
        return r;
    }
    // A pending Pen anchor is gesture state; Edit › Undo must match the keyboard.
    if id == "edit.undo" && crate::vector_ui::pen_undo_last_point(app) {
        return Ok(Value::Null);
    }
    // Image › Crop while the Crop tool has a pending frame commits that frame (#1918).
    if id == "image.crop" && crate::crop_ui::pending(app) {
        crate::canvas::commit_crop(app);
        return Ok(json!({"committed": true}));
    }
    if let Some(result) = crate::panel_docking::command(app, id, &params) {
        return result;
    }
    // Help › Discord, website, GitHub, Report an Issue.
    if let Some(url) = crate::links::url_for(id) {
        return Ok(crate::links::open(app, ctx, url));
    }
    if id == "view.proofSetup.custom" {
        return Ok(json!({"dialog": crate::filter_dialog::open(app, "view.proofSetup")}));
    }
    // Image › Analysis tools/dialogs, Measurement Log and Notes panels, File › Import › Notes.
    if let Some(r) = crate::analysis_ui::menu(app, id, &params) {
        return r;
    }
    // Workspace New/Delete/Lock, Modifier Keys, custom pixel aspect, Extras/32-bit options.
    if let Some(r) = crate::workspace_ui::menu(app, id, &params) {
        return r;
    }
    // Window › Gradients, Patterns, Styles, Shapes, Tool Presets, Clone Source.
    if let Some(r) = crate::preset_panels::menu(app, id, &params) {
        return r;
    }
    // Character/Paragraph Styles, Glyphs, Edit › Check Spelling dialog.
    if let Some(r) = crate::type_panels_ui::menu(app, id, &params) {
        return r;
    }
    // Image > Variables (Define / Data Sets), Apply Data Set.
    if let Some(r) = crate::variables_ui::menu(app, id, &params) {
        return r;
    }
    // Window > Timeline panel.
    if let Some(r) = crate::timeline_ui::menu(app, id, &params) {
        return r;
    }
    // Filter › Plug-ins (installed WebAssembly plug-ins, Install Plug-in…).
    if let Some(r) = crate::plugin_ui::menu(app, id, &params) {
        return r;
    }
    if id == "window.panel.brushSettings" {
        app.ui.brush_tab = 0;
    }
    // View/Window/Type shell items, and dialogs/pickers in front of File commands.
    // Edit › Preferences, Keyboard Shortcuts, Color Settings and other Edit dialogs.
    if let Some(r) = crate::prefs_ui::invoke(app, ctx, id, &params) {
        return r;
    }
    // Save for Web, Print and the other File-menu dialogs added with slices.
    if let Some(r) = crate::file_ui::invoke(app, ctx, id, &params) {
        return r;
    }
    if let Some(r) = crate::view_cmds::invoke(app, ctx, id, &params) {
        return r;
    }
    // Liquify dialog, Puppet Warp and Perspective Warp modes (and their control params).
    if let Some(r) = crate::distort_ui::menu(app, ctx, id, &params) {
        return r;
    }
    // Camera Raw Filter dialog (and its control params).
    if let Some(r) = crate::camera_raw_ui::menu(app, ctx, id, &params) {
        return r;
    }
    if let Some(r) = crate::wide_angle_ui::menu(app, ctx, id, &params) {
        return r;
    }
    if let Some(profile) = proof_preset(id) {
        // View › Proof Setup presets: set the proof profile and turn Proof Colors on.
        app.run("view.proofSetup", json!({"profile": profile}))?;
        return app.run("view.proofColors", json!({"on": true}));
    }
    if let Some(alias) = panel_alias(id) {
        return invoke(app, ctx, alias, params);
    }
    if let Some(ws) = workspace_name(id) {
        // Reset re-applies the current workspace; Essentials is the default layout.
        match (ws, id) {
            (_, "window.workspace.resetWorkspace") => {}
            ("", _) => app.ui.workspace = "Essentials".into(),
            (name, _) => app.ui.workspace = name.into(),
        }
        apply_workspace(app);
        return Ok(json!({"workspace": app.ui.workspace}));
    }
    match id {
        // ⌘↩ / Ctrl+Enter (#306): load the path selected in the Paths panel (or the one being
        // drawn) as a selection.
        "path.toSelection" if params.get("name").is_none() => crate::vector_ui::path_to_selection(app, params),
        // Layer › Rename Layer from a menu starts in-place renaming in the Layers panel.
        "layer.renameLayer" if params.get("name").is_none() => {
            let st = app.session.active().ok_or("no document")?;
            let id = params.get("layer").and_then(Value::as_u64).or(st.active_layer.map(|l| l.0)).ok_or("no active layer")?;
            let name = st.doc.layer(photocraft_doc::LayerId(id)).map(|l| l.name.clone()).ok_or("no such layer")?;
            // Another rename in progress is committed first (#314).
            if let Some((cmd, p)) = crate::layer_row_ui::start_rename(ctx, id, &name) {
                app.run(&cmd, p)?;
            }
            Ok(Value::Null)
        }
        "layer.duplicate" if params.as_object().is_none_or(|o| o.is_empty()) => crate::duplicate_layer_ui::open(app),
        "file.new" if params.as_object().is_none_or(|o| o.is_empty()) => {
            let fields = app.new_document_fields();
            let d = app.ui.open_dialog(DialogKind::NewDocument, fields);
            Ok(json!({"dialog": d}))
        }
        "file.open" => {
            if let Some(path) = params.get("path").and_then(Value::as_str) {
                open_path(app, path)
            } else {
                app.open_dialog_file()
            }
        }
        "file.import.videoFramesToLayers" if params.get("path").and_then(Value::as_str).is_none() => {
            app.ask_folder(|app, path| app.run("file.import.videoFramesToLayers", json!({"path": path})).map_err(|e| e.to_string()))
        }
        "file.save" => {
            // Writes back only to a layered file that holds the whole document; anything else
            // goes through Save As, so the original is never overwritten with less (#3806).
            let tga_bits = crate::targa_options_ui::bits_param(&params, id)?;
            match params.get("path").and_then(Value::as_str) {
                Some(path) => app.save_as_with(Some(path.to_string()), tga_bits),
                None if app.services.default_save.is_some() => app.save(None),
                None => {
                    let own = app.session.active().and_then(|d| d.path.clone().map(|p| (photocraft_engine::file_cmds::holds_at(&d.doc, &p), p, d)));
                    let own = own.filter(|(_, path, _)| !path.starts_with("save-as/"));
                    match own {
                        Some((true, path, d)) => {
                            // Another app changed the file since it was opened or saved: ask first (#6596).
                            let doc = d.doc.id;
                            crate::disk_change_prompt::intercept(app, doc, &path, false).unwrap_or_else(|| app.save_in_place(path))
                        }
                        Some((false, path, d)) if photocraft_engine::file_cmds::saves_in_place(&path) => {
                            // Its own file is layered but can't hold what the document has now.
                            let losses = photocraft_engine::file_cmds::losses_at(&d.doc, &path);
                            let name = crate::file_open::display_name(&path);
                            let title = crate::i18n::fmt(tl!("{name} can't keep everything in this document"), &[("name", name.as_str())]);
                            let mut lines = losses;
                            lines.push(tl!("Choose a format that keeps it, such as Photoshop (PSD), or save a copy.").to_string());
                            crate::notices::post(app, title, lines, false, None);
                            app.save_as_with(None, tga_bits)
                        }
                        // A flat file asks for its name; confirming its own file records a
                        // plain Save that writes the file an action is played on (#3531).
                        _ => app.save_dialog_flow(None, tga_bits, true),
                    }
                }
            }
        }
        "file.exit" => {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            Ok(Value::Null)
        }
        "file.clearRecent" => {
            app.clear_recent();
            Ok(Value::Null)
        }
        "file.removeRecent" => {
            let path = params.get("path").and_then(Value::as_str).ok_or("file.removeRecent needs a \"path\"")?;
            app.remove_recent(path);
            Ok(Value::Null)
        }
        id if id.starts_with("file.openRecent.") => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match id.rsplit('.').next().and_then(|s| s.parse::<usize>().ok()).and_then(|i| app.ui.recent_files.get(i).cloned()) {
                    Some(p) => open_path(app, &p),
                    None => Err("no such recent file".to_string()),
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = app;
                let message = "Open Recent is unavailable on the web";
                Err(tl!(message).to_string())
            }
        }
        "file.saveAs" => {
            let tga_bits = crate::targa_options_ui::bits_param(&params, id)?;
            app.save_as_with(params.get("path").and_then(Value::as_str).map(str::to_string), tga_bits)
        }
        "view.zoomIn" | "view.zoomOut" | "view.fitOnScreen" | "view.actualPixels" => app.run(id, params),
        "window.newWindowForDocument" => {
            let doc = app.session.active_index().ok_or("no document")?;
            let wid = app.ui.alloc_id();
            let view = app.ui.views[doc].clone();
            app.ui.windows.push(crate::state::DocWindow { id: wid, document: doc, view, open: true, position: None });
            Ok(json!({"window": wid}))
        }
        "window.theme.toggle" => {
            crate::prefs_ui::cycle_appearance(app, ctx);
            Ok(Value::Null)
        }
        "view.darkenInterface" | "view.lightenInterface" => {
            crate::prefs_ui::step_brightness(app, ctx, id == "view.lightenInterface")?;
            Ok(json!({"theme": app.ui.theme.id()}))
        }
        "window.theme.system" => {
            app.run("prefs.set", json!({"path": "interface.appearanceMode", "value": "auto"}))?;
            crate::prefs_ui::sync_appearance(app, ctx);
            Ok(Value::Null)
        }
        "window.theme.pro"
        | "window.theme.proMedium"
        | "window.theme.proLightGray"
        | "window.theme.proLight"
        | "window.theme.studio"
        | "window.theme.studioLight"
        | "window.theme.classic"
        | "window.theme.solarizedDark"
        | "window.theme.adwaita"
        | "window.theme.adwaitaDark"
        | "window.theme.breezeLight"
        | "window.theme.breezeDark"
        | "window.theme.softLight"
        | "window.theme.softDark" => {
            let k = crate::theme::ThemeKind::from_name(&id["window.theme.".len()..]).unwrap_or_default();
            app.set_theme(ctx, k);
            Ok(Value::Null)
        }
        // The macOS app menu's Language (`native_menu::language_node`).
        i if i.starts_with(crate::native_menu::LANGUAGE_PREFIX) => {
            let code = &i[crate::native_menu::LANGUAGE_PREFIX.len()..];
            if code != "auto" && !crate::i18n::Lang::all().any(|l| l.code() == code) {
                return Err(format!("unknown UI language `{code}`"));
            }
            app.run("prefs.set", json!({"values": {"interface.language": code}}))
        }
        "edit.search" => {
            app.ui.palette_open = !app.ui.palette_open;
            Ok(Value::Null)
        }
        "help.about" => Ok(json!({"dialog": app.ui.open_dialog(DialogKind::About, Default::default())})),
        "help.systemInfo" => {
            let mut fields = serde_json::Map::new();
            fields.insert("systemInfo".into(), json!(true));
            let dialog = app.ui.open_dialog(DialogKind::About, fields);
            Ok(json!({"dialog": dialog, "info": crate::gpu_status::system_info_json(app)}))
        }
        "file.export.exportAs" => Ok(json!({"dialog": crate::export_dialog::open(app)?})),
        "file.export.quickExportAsPng" => crate::export_dialog::quick_export_png(app),
        // Render Video exports a sequence of frames into a folder, not a named file.
        // Keep explicit directory arguments from automation and the control channel unchanged.
        "file.export.renderVideo" if params.get("dir").is_none() => {
            #[cfg(target_arch = "wasm32")]
            {
                Err("Render Video to a local directory is unavailable on the web".into())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let doc = app.session.active().ok_or("no document")?.doc.id;
                let mut render_params = params.as_object().cloned().unwrap_or_default();
                app.ask_folder(move |app, dir| {
                    render_params.insert("dir".into(), json!(dir));
                    app.with_document(doc, |app| app.run("file.export.renderVideo", Value::Object(render_params)))
                })
            }
        }
        // Layer › Export As… / Quick Export as PNG: the export pipeline on just the active layer.
        l if matches!(l, "layer.exportAs" | "layer.quickExportAsPng") && params.as_object().is_none_or(|o| o.is_empty()) => {
            let layer = app.session.active().and_then(|d| d.active_layer).ok_or("no active layer")?;
            if l == "layer.exportAs" {
                // Several layers (artboards, say) selected: one file each (#3629).
                let layers = app.session.active().map(|d| d.selected_layers()).unwrap_or_default();
                if layers.len() > 1 {
                    return Ok(json!({"dialog": crate::export_dialog::open_layers(app, &layers)?}));
                }
                Ok(json!({"dialog": crate::export_dialog::open_layer(app, layer)?}))
            } else {
                crate::export_dialog::quick_export_selected_layers_png(app)
            }
        }
        "layer.layerStyle.blendingOptions" if params.as_object().is_none_or(|o| o.is_empty()) => {
            crate::layer_style::open(app, Some(crate::layer_style::BLENDING)).map(|d| json!({"dialog": d})).ok_or_else(|| "no active layer".to_string())
        }
        // ⇧⌘> / ⇧⌘<: the selected characters while typing, else the whole type layer.
        "type.increaseSize" | "type.decreaseSize" | "type.increaseSizeMore" | "type.decreaseSizeMore" if params.as_object().is_none_or(|o| o.is_empty()) => {
            let p = crate::type_tool::with_target(app, json!({})).ok_or("the active layer is not a type layer")?;
            app.run(id, p)
        }
        // Formatting menus follow an inline text selection when editing. Outside an edit,
        // snapshot the selected type layers; explicit automation targets retain their scope.
        t if (t.starts_with("type.antiAlias.")
            || t.starts_with("type.orientation.")
            || t.starts_with("type.openType.")
            || matches!(t, "type.warpText" | "type.loadDefaultTypeStyles"))
            && params.get("layer").is_none()
            && params.get("layers").is_none()
            && params.get("range").is_none() =>
        {
            let Some(mut p) = crate::type_tool::formatting_params(app) else { return app.run(id, params) };
            if let Some(props) = params.as_object() {
                for (key, value) in props {
                    p[key] = value.clone();
                }
            }
            app.run(id, p)
        }
        "type.editText" => {
            crate::type_tool::edit_active(app)?;
            if let Some(focus) = ctx.memory(|m| m.focused()) {
                ctx.memory_mut(|m| m.surrender_focus(focus));
            }
            Ok(Value::Null)
        }
        // Only interactive creation opens the Color Picker; it creates the layer on OK,
        // so Cancel leaves neither an orphan layer nor an undo/redo history entry.
        // Explicit parameters continue straight to the engine.
        "layer.newFillLayer.solidColor" if params.as_object().is_none_or(|o| o.is_empty()) => {
            let dialog = crate::solid_fill_ui::open_new(app).ok_or("no document is open")?;
            Ok(json!({"dialog": dialog}))
        }
        // Solid Color fills and solid-colour shapes use the Color Picker, Pattern fills the Pattern
        // Fill dialog; other adjustment / fill / shape controls use Properties.
        "layer.layerContentOptions" => {
            if let Some(dialog) = crate::solid_fill_ui::open(app).or_else(|| crate::shape_fill_ui::open(app)) {
                return Ok(json!({"dialog": dialog}));
            }
            if let Some(dialog) = crate::pattern_fill_ui::open(app) {
                return Ok(json!({"dialog": dialog}));
            }
            let r = app.run(id, params)?;
            app.ui.panels.properties = true;
            app.ui.dock_tabs.properties = 0;
            Ok(r)
        }
        // Edit › Stroke…: open its full options dialog when called from menus/context
        // menus. Explicit params from Actions, CLI, MCP and control channel execute directly.
        crate::stroke_ui::COMMAND if params.as_object().is_none_or(|o| o.is_empty()) => {
            if let Some(Err(why)) = photocraft_engine::commands::find(id).map(|c| (c.enabled)(&app.session)) {
                return Err(why);
            }
            Ok(json!({"dialog": crate::stroke_ui::open(app)}))
        }
        // Channels › Channel Options… without params: the dialog on the targeted alpha/spot
        // channel, or Quick Mask Options while in Quick Mask (with params: the engine directly).
        crate::channel_options_ui::COMMAND if params.as_object().is_none_or(|o| o.is_empty()) => {
            let st = app.session.active().ok_or("no document is open")?;
            let channel = match st.channel_view.target {
                _ if st.doc.quick_mask.is_some() => json!("quickMask"),
                photocraft_engine::channel_cmds::ChannelTarget::Alpha(i) => json!(i),
                _ => return Err("target an alpha or spot channel first".into()),
            };
            let dialog = crate::channel_options_ui::open(app, &channel).ok_or("no such channel")?;
            Ok(json!({"dialog": dialog}))
        }
        // Channels › New Spot Channel… without params: its dialog (with params: the engine).
        crate::channel_options_ui::NEW_SPOT if params.as_object().is_none_or(|o| o.is_empty()) => {
            let dialog = crate::channel_options_ui::open_new_spot(app).ok_or("no document is open")?;
            Ok(json!({"dialog": dialog}))
        }
        // Photoshop's "Select and Mask…" is the engine's select.refineEdge.
        // Select › Color Range… from the menu: the dialog (with params: the engine directly).
        "select.colorRange" if params.as_object().is_none_or(|o| o.is_empty()) => Ok(json!({"dialog": crate::color_range_ui::open(app)})),
        // Layer › New Fill Layer › Pattern… from the menu: the Pattern Fill dialog (with params:
        // the engine directly).
        photocraft_engine::pattern_fill_cmds::CREATE if params.as_object().is_none_or(|o| o.is_empty()) => {
            Ok(json!({"dialog": crate::pattern_fill_ui::open_new(app)?}))
        }
        // Edit › Fill… from the menu or its shortcuts: the Fill dialog (with params: the engine).
        crate::fill_ui::COMMAND if params.as_object().is_none_or(|o| o.is_empty()) => {
            if let Some(Err(why)) = photocraft_engine::commands::find(id).map(|c| (c.enabled)(&app.session)) {
                return Err(why);
            }
            Ok(json!({"dialog": crate::fill_ui::open(app)}))
        }
        "select.selectAndMask" => Ok(json!({"dialog": crate::filter_dialog::open(app, "select.refineEdge")})),
        // Select › Transform Selection from the menu: the interactive box (with params: the engine).
        "select.transformSelection" if params.as_object().is_none_or(|o| o.is_empty()) => {
            crate::transform_tool::begin_selection(app, ctx).map(|_| json!({"transform": app.ui.transform}))
        }
        // While editing type, the clipboard rows and Select > All work on the text (#1355, #2893).
        "edit.cut" if app.ui.text_edit.is_some() => {
            crate::type_tool::cut_selection(app, ctx);
            Ok(Value::Null)
        }
        "edit.copy" if app.ui.text_edit.is_some() => {
            crate::type_tool::copy_selection(app, ctx);
            Ok(Value::Null)
        }
        "edit.paste" if app.ui.text_edit.is_some() => {
            crate::type_tool::request_paste(ctx);
            Ok(Value::Null)
        }
        "select.all" if app.ui.text_edit.is_some() => {
            crate::type_tool::select_all(app);
            Ok(Value::Null)
        }
        "edit.paste" if params.as_object().is_none_or(|o| o.is_empty()) => {
            // Photoshop: paste in place when the copied area is visible, else centred in the view;
            // images from other apps are always centred.
            app.import_os_clipboard();
            app.clip_read_for_paste = true;
            let external = app.clip_external;
            let visible = !external
                && app.session.active_index().zip(app.session.clipboard.as_ref()).is_some_and(|(i, clip)| {
                    let v = &app.ui.views[i];
                    let pz = app.point_zoom();
                    let (hw, hh) = (app.last_canvas_rect.width() / 2.0 / pz / app.ui.view.display_aspect(), app.last_canvas_rect.height() / 2.0 / pz);
                    let r =
                        photocraft_geom::Rect::new((v.center[0] - hw) as i32, (v.center[1] - hh) as i32, (v.center[0] + hw) as i32, (v.center[1] + hh) as i32);
                    !clip.bounds.intersect(&r).is_empty()
                });
            let p = match app.session.active_index() {
                Some(i) if !visible => json!({"center": app.ui.views[i].center}),
                _ => json!({}),
            };
            app.run("edit.paste", p)
        }
        "view.rulers" | "view.show.grid" | "view.show.guides" | "view.snap" | "view.lockGuides" => {
            let e = &mut app.ui.extras;
            let slot = match id {
                "view.rulers" => &mut e.rulers,
                "view.show.grid" => &mut e.grid,
                "view.show.guides" => &mut e.guides,
                "view.snap" => &mut e.snap,
                _ => &mut e.lock_guides,
            };
            *slot = !*slot;
            let on = *slot;
            if id == "view.rulers" {
                crate::prefs_ui::remember_rulers(app, on);
            }
            Ok(json!(on))
        }
        "edit.freeTransform"
        | "edit.transform.scale"
        | "edit.transform.rotate"
        | "edit.transform.skew"
        | "edit.transform.distort"
        | "edit.transform.perspective" => {
            // While a box is up, Scale / Rotate / Skew / Distort / Perspective (and Free Transform)
            // switch its mode; otherwise they start one in that mode.
            if app.ui.transform.as_ref().is_some_and(|t| t.content_aware.is_some()) {
                return Err(crate::transform_tool::CAS_ONLY_SCALES.into());
            }
            if app.ui.transform.is_none() {
                crate::transform_tool::begin(app, ctx)?;
            }
            if let Some(t) = app.ui.transform.as_mut() {
                t.mode = crate::state::TransformMode::for_command(id);
            }
            Ok(json!({"transform": app.ui.transform}))
        }
        "edit.freeTransformCopy" => crate::transform_tool::begin_copy(app, ctx).map(|_| json!({"transform": app.ui.transform})),
        // Edit › Transform › Warp from the menu: interactive Warp mode (with params: the engine).
        "edit.transform.warp" | "layer.smartObjects.warp" if params.as_object().is_none_or(|o| o.is_empty()) => {
            crate::transform_tool::begin_warp(app, ctx).map(|_| json!({"transform": app.ui.transform}))
        }
        // Split warps edit the mesh of an active Warp session.
        "edit.transform.splitWarpCrosswise"
        | "edit.transform.splitWarpHorizontally"
        | "edit.transform.splitWarpVertically"
        | "edit.transform.removeWarpSplit"
            if app.ui.transform.as_ref().is_some_and(|t| t.warp.is_some()) && params.get("warp").is_none() =>
        {
            let at = params.get("at").and_then(Value::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
            crate::transform_tool::split(app, id, at).map(|_| json!({"transform": app.ui.transform}))
        }
        "edit.transform.warpGrid" if app.ui.transform.as_ref().is_some_and(|t| t.warp.is_some()) && params.get("warp").is_none() => {
            crate::transform_tool::edit_session_warp(app, id, &params).map(|_| json!({"transform": app.ui.transform}))
        }
        // Layer › New › Layer… (⇧⌘N) asks first, as in Photoshop; with params it runs as is.
        crate::new_layer_ui::COMMAND if params.as_object().is_none_or(|o| o.is_empty()) => Ok(json!({"dialog": crate::new_layer_ui::open(app)?})),
        sz if crate::sizing::is_sizing(sz) && params.as_object().is_none_or(|o| o.is_empty()) => {
            Ok(json!({"dialog": crate::sizing::open(app, sz).ok_or("no document")?}))
        }
        "view.newGuide" if params.as_object().is_none_or(|o| o.is_empty()) => Ok(json!({"dialog": crate::new_guide_ui::open(app)})),
        fl if crate::filter_dialog::has_dialog(fl) && params.as_object().is_none_or(|o| o.is_empty()) => {
            Ok(json!({"dialog": crate::filter_dialog::open(app, fl)}))
        }
        a if crate::adjust_dialog::has_dialog(a) && params.as_object().is_none_or(|o| o.is_empty()) => {
            Ok(json!({"dialog": crate::adjust_dialog::open(app, a).ok_or("no document")?}))
        }
        "window.togglePanels" => {
            // Tab hides the Tools panel, the options bar and the dock; Tab again shows what it hid.
            let p = &mut app.ui.panels;
            match p.hidden_by_tab.take() {
                Some([toolbar, options, dock]) if !(p.toolbar || p.options_bar || p.dock) => {
                    (p.toolbar, p.options_bar, p.dock) = (toolbar, options, dock);
                }
                _ if p.toolbar || p.options_bar || p.dock => {
                    p.hidden_by_tab = Some([p.toolbar, p.options_bar, p.dock]);
                    (p.toolbar, p.options_bar, p.dock) = (false, false, false);
                }
                _ => (p.toolbar, p.options_bar, p.dock) = (true, true, true),
            }
            Ok(Value::Null)
        }
        "window.nextDocument" | "window.previousDocument" => {
            // Tabs are in document order, so the next tab is the next document, wrapping around.
            let n = app.session.documents().len();
            let at = app.session.active_index().ok_or("no document open")?;
            let to = if id == "window.nextDocument" { (at + 1) % n } else { (at + n - 1) % n };
            app.session.set_active(to);
            app.jobs.focus = None;
            Ok(json!({"document": to}))
        }
        id if let Some(index) = id.strip_prefix(DOCUMENT_ITEM) => {
            let n = app.session.documents().len();
            let to = index.parse::<usize>().ok().filter(|i| *i < n).ok_or("no such document")?;
            app.session.set_active(to);
            app.jobs.focus = None;
            Ok(json!({"document": to}))
        }
        t if t.starts_with("window.toggle.") => {
            // A shown but collapsed dock group is expanded rather than hidden (#129).
            if let Some(g) = crate::dock::Group::from_key(&t["window.toggle.".len()..]).filter(|g| g.shown(&app.ui.panels) && app.ui.dock.is_collapsed(*g)) {
                crate::dock::reveal(app, g);
                return Ok(Value::Null);
            }
            let p = &mut app.ui.panels;
            let slot = match &t["window.toggle.".len()..] {
                "layers" => &mut p.layers,
                "history" => &mut p.history,
                "properties" => &mut p.properties,
                "color" => &mut p.color,
                "navigator" => &mut p.navigator,
                "toolbar" => &mut p.toolbar,
                "options" => &mut p.options_bar,
                "brushSettings" => &mut p.brush_settings,
                "brushes" => &mut p.brushes,
                "dock" => &mut p.dock,
                _ => return Err(format!("unknown panel in {t}")),
            };
            *slot = !*slot;
            if let Some(g) = crate::dock::Group::from_key(&t["window.toggle.".len()..]).filter(|g| g.shown(&app.ui.panels)) {
                crate::dock::reveal(app, g);
            }
            Ok(Value::Null)
        }
        // Layer › Layer Style › <effect>… opens the Layer Style dialog on that effect.
        ls if params.as_object().is_none_or(|o| o.is_empty())
            && ls.strip_prefix("layer.layerStyle.").is_some_and(|k| crate::layer_style::KINDS.iter().any(|(kind, _)| *kind == k)) =>
        {
            let kind = &ls["layer.layerStyle.".len()..];
            crate::layer_style::open(app, Some(kind)).map(|d| json!({"dialog": d})).ok_or_else(|| "no active layer".to_string())
        }
        _ => app.run(id, params),
    }
}

fn open_path(app: &mut PhotocraftApp, path: &str) -> Result<Value, String> {
    app.open_path(path).map(|w| json!({"warnings": w}))
}

pub fn is_enabled(app: &PhotocraftApp, id: &str) -> bool {
    if crate::tool_shortcuts::tool(id).is_some() {
        return true;
    }
    if id == "ui.symmetryTransform" {
        return crate::symmetry_ui::can_transform(app);
    }
    // Draft Pen points can be undone even when the document has no History entries.
    if id == "edit.undo" && app.ui.pen.as_ref().is_some_and(|pen| !pen.knots.is_empty()) {
        return true;
    }
    // Photoshop greys these for the Background layer, other layer kinds or single-layer documents.
    if crate::enable_rules::disabled(app, id) {
        return false;
    }
    // A pending crop greys what Photoshop greys during one (#1918).
    if crate::crop_ui::blocks(app, id) {
        return false;
    }
    // Filters stay live on a type layer: choosing one asks to rasterize it (#4303).
    if crate::rasterize_prompt::filter_needs(app, id).is_some() {
        return true;
    }
    if let Some(e) = crate::workspace_ui::is_enabled(app, id) {
        return e;
    }
    if crate::analysis_ui::handles(id) {
        return true;
    }
    if crate::preset_panels::handles(id) || crate::type_panels_ui::handles(id) || crate::timeline_ui::handles(id) {
        return true;
    }
    if let Some(e) = crate::view_cmds::is_enabled(app, id) {
        return e;
    }
    if let Some(e) = crate::plugin_ui::is_enabled(app, id) {
        return e;
    }
    if let Some(e) = crate::transform_tool::is_enabled(app, id) {
        return e;
    }
    if let Some(e) = crate::crop_ui::is_enabled(app, id) {
        return e;
    }
    match id {
        "window.panel.activate" | "window.panel.layout" => true,
        "window.panel.group" | "window.panel.move" | "window.panel.float" | "window.panel.dock" | "window.panel.close" => !app.session.prefs().workspace_locked,
        "file.open" | "file.exit" | "file.clearRecent" | "file.removeRecent" | "help.about" | "help.systemInfo" | "edit.search" => true,
        i if i.starts_with("file.openRecent.") => true,
        i if i.starts_with(DOCUMENT_ITEM) => true,
        i if crate::links::url_for(i).is_some() => true,
        i if i.starts_with("window.theme.") => true,
        // Interface brightness works with or without a document, as in Photoshop.
        "view.darkenInterface" | "view.lightenInterface" => true,
        "file.save" | "file.saveAs" | "file.export.exportAs" | "file.export.quickExportAsPng" => {
            app.session.active().is_some() && app.services.export.is_some()
        }
        i if i.starts_with("window.toggle.") => true,
        "window.togglePanels" => true,
        // With one document open the key stays quiet, as in Photoshop, rather than reporting why.
        "window.nextDocument" | "window.previousDocument" => app.session.active().is_some(),
        i if panel_alias(i).is_some() || workspace_name(i).is_some() => true,
        i if proof_preset(i).is_some() => app.session.active().is_some(),
        // "Custom…" is the full Proof Setup dialog.
        "view.proofSetup.custom" => app.session.active().is_some(),
        "view.rulers" | "view.show.grid" | "view.show.guides" | "view.snap" | "view.lockGuides" => true,
        "edit.cut" | "edit.copy" if app.ui.text_edit.is_some() => crate::type_tool::selected_range(app).is_some(),
        "edit.paste" | "select.all" if app.ui.text_edit.is_some() => true,
        // An image copied in another app can only be seen by reading the OS clipboard, which happens
        // on an explicit paste: with a clipboard service these stay enabled. Paste and New from
        // Clipboard need no document (with none open, Paste makes one); Paste in Place does.
        "edit.paste" | "file.newFromClipboard" => app.session.is_enabled(id) || app.services.clipboard_get_image.is_some(),
        "edit.pasteSpecial.pasteInPlace" => app.session.is_enabled(id) || (app.services.clipboard_get_image.is_some() && app.session.active().is_some()),
        "select.selectAndMask" => app.session.is_enabled("select.refineEdge"),
        "type.editText" => app
            .session
            .active()
            .and_then(|s| s.active_layer.and_then(|id| s.doc.layer(id)))
            .is_some_and(|l| matches!(l.content, photocraft_doc::LayerContent::Text(_))),
        "select.transformSelection" => app.ui.transform.is_none() && app.session.is_enabled("select.transformSelection"),
        // ⇧[ / ⇧] only step the hardness of a tool that paints with the brush tip, as in Photoshop.
        "tools.decreaseBrushHardness" | "tools.increaseBrushHardness" => app.ui.tool.is_brushlike(),
        i if (i.starts_with("view.zoom") || i == "view.fitOnScreen" || i == "view.actualPixels") || i == "window.newWindowForDocument" => {
            app.session.active().is_some()
        }
        "edit.freeTransformCopy" => app.ui.transform.is_none() && app.session.active().and_then(|s| s.active_layer).is_some(),
        "edit.freeTransform"
        | "edit.transform.scale"
        | "edit.transform.rotate"
        | "edit.transform.skew"
        | "edit.transform.distort"
        | "edit.transform.perspective" => match &app.ui.transform {
            Some(t) => t.warp.is_none(),
            None => app.session.active().and_then(|s| s.active_layer).is_some(),
        },
        // A targeted layer mask enables what edits it (Invert on an adjustment layer's mask).
        i => app.session.is_enabled_with(i, &app.with_mask_target(i, Value::Null)),
    }
}

/// Image › Mode items that carry a checkmark.
const MODE_CHECKS: [&str; 11] = ["rgb", "grayscale", "cmyk", "lab", "multichannel", "indexedColor", "bitmap", "duotone", "bits8", "bits16", "bits32"];

/// Is a UI-level panel toggle currently on (for checkmarks)?
fn checked(app: &PhotocraftApp, id: &str) -> Option<bool> {
    if let Some(checked) = crate::panel_docking::checked(app, id) {
        return Some(checked);
    }
    use photocraft_doc::{ColorMode, SampleType};
    if let Some(name) = id.strip_prefix("window.theme.").filter(|n| *n != "toggle") {
        let auto = app.session.prefs().interface.appearance_mode == photocraft_engine::prefs::AppearanceMode::Auto;
        return Some(if name == "system" { auto } else { !auto && crate::theme::ThemeKind::from_name(name) == Some(app.ui.theme) });
    }
    if let Some(c) = crate::view_cmds::checked(app, id) {
        return Some(c);
    }
    if let Some(c) = crate::analysis_ui::checked(app, id).or_else(|| crate::workspace_ui::checked(app, id)).or_else(|| crate::file_ui::checked(app, id)) {
        return Some(c);
    }
    if let Some(c) = crate::preset_panels::checked(app, id) {
        return Some(c);
    }
    if let Some(c) = crate::timeline_ui::checked(app, id) {
        return Some(c);
    }
    if let Some(c) = crate::type_panels_ui::checked(app, id) {
        return Some(c);
    }
    if let Some(alias) = panel_alias(id) {
        return checked(app, alias);
    }
    // Check items stay check items with no document open (`Some(false)`, not `None`): a native
    // menu can't change an item's kind in place, so a change would rebuild the whole menu.
    if id == "select.isolateLayers" {
        return Some(app.session.active().is_some_and(|d| !d.isolated_layers.is_empty()));
    }
    if id == "view.proofColors" || id == "view.gamutWarning" {
        let Some(d) = app.session.active() else { return Some(false) };
        let pv = app.session.color.proof(d.doc.id);
        return Some(if id == "view.proofColors" { pv.enabled } else { pv.gamut_warning });
    }
    if id.starts_with("window.workspace.") && id != "window.workspace.resetWorkspace" {
        let want = workspace_name(id)?;
        return Some(app.ui.workspace == if want.is_empty() { "Essentials" } else { want });
    }
    if let Some(m) = id.strip_prefix("image.mode.") {
        let Some(d) = app.session.active().map(|s| &s.doc) else {
            return MODE_CHECKS.contains(&m).then_some(false);
        };
        return match m {
            "rgb" => Some(d.mode == ColorMode::Rgb),
            "grayscale" => Some(d.mode == ColorMode::Grayscale),
            "cmyk" => Some(d.mode == ColorMode::Cmyk),
            "lab" => Some(d.mode == ColorMode::Lab),
            "multichannel" => Some(d.mode == ColorMode::Multichannel),
            "indexedColor" => Some(d.mode == ColorMode::Indexed),
            "bitmap" => Some(d.mode == ColorMode::Bitmap),
            "duotone" => Some(d.mode == ColorMode::Duotone),
            "bits8" => Some(d.depth == SampleType::U8),
            "bits16" => Some(d.depth == SampleType::U16),
            "bits32" => Some(d.depth == SampleType::F32),
            _ => None,
        };
    }
    let e = &app.ui.extras;
    match id {
        "view.rulers" => return Some(e.rulers),
        "view.show.grid" => return Some(e.grid),
        "view.show.guides" => return Some(e.guides),
        "view.snap" => return Some(e.snap),
        "view.lockGuides" => return Some(e.lock_guides),
        _ => {}
    }
    let p = &app.ui.panels;
    Some(match id {
        "window.toggle.layers" => p.layers,
        "window.toggle.history" => p.history,
        "window.toggle.properties" => p.properties,
        "window.toggle.color" => p.color,
        "window.toggle.navigator" => p.navigator,
        "window.toggle.toolbar" => p.toolbar,
        "window.toggle.dock" => p.dock,
        "window.togglePanels" => p.toolbar || p.options_bar || p.dock,
        "window.toggle.options" => p.options_bar,
        "window.toggle.brushSettings" => p.brush_settings,
        "window.toggle.brushes" => p.brushes,
        _ => return None,
    })
}

/// "Reveal in Finder" named after the file manager the engine opens on this platform (UI-217-19):
/// Explorer on Windows, a plain folder elsewhere (xdg-open, the web). English action key; each
/// display translates it.
pub(crate) fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Reveal in Finder"
    } else if cfg!(windows) {
        "Show in Explorer"
    } else {
        "Show in Folder"
    }
}

/// Translate the fixed command label and substitute the currently configured export format.
/// Reuse the existing translated PNG sentence, so dynamic formats work in every UI language.
fn translated_menu_label(lang: crate::i18n::Lang, item: &MenuItem) -> String {
    // A document's name is shown as it is.
    if item.id.starts_with(DOCUMENT_ITEM) {
        return item.label.clone();
    }
    if item.id == "file.export.quickExportAsPng"
        && let Some(fmt) = item.label.strip_prefix("Quick Export as ")
    {
        return crate::i18n::tr_id(lang, &item.id, "Quick Export as PNG").replace("PNG", fmt);
    }
    let label = crate::i18n::tr_id(lang, &item.id, &item.label);
    if cfg!(feature = "harmonyos-ui") { label.replace("PhotoCraft", crate::brand::APP_NAME) } else { label.to_string() }
}

/// Menu tree entry for rendering and for `ui.inspect`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub path: Vec<String>,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub checked: Option<bool>,
    /// Edit › Menus colour (red, orange, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Is `id` implemented by the engine or the shell (a live menu item)? Shared by the menus and
/// the parity report ([`crate::parity`]).
pub fn is_live(id: &str) -> bool {
    static UI_IDS: std::sync::OnceLock<std::collections::HashSet<&'static str>> = std::sync::OnceLock::new();
    photocraft_engine::commands::find(id).is_some()
        || crate::tool_shortcuts::tool(id).is_some()
        || UI_IDS.get_or_init(|| UI_COMMANDS.iter().map(|c| c.0).collect()).contains(id)
        || panel_alias(id).is_some()
        || workspace_name(id).is_some()
        || proof_preset(id).is_some()
        || id == "view.proofSetup.custom"
        || crate::view_cmds::handles(id)
        || crate::analysis_ui::handles(id)
        || crate::workspace_ui::handles(id)
        || crate::preset_panels::handles(id)
        || crate::type_panels_ui::handles(id)
        || crate::timeline_ui::handles(id)
}

/// Commands outside the catalogue that belong right after a catalogue item: `(id, after)`.
/// The command takes that item's submenu path, so it joins Photoshop's submenu even where the
/// catalogue spells it differently (Filter › "Other…").
const PLACE_AFTER: &[(&str, &str)] = &[
    ("file.newFromClipboard", "file.new"),
    ("filter.render.relight", "filter.render.lightingEffects"),
    ("view.resetView", "view.flipHorizontal"),
    ("filter.other.colorToAlpha", "filter.other.offset"),
];

pub fn menu_items(app: &PhotocraftApp) -> Vec<MenuItem> {
    // 1) Photoshop's full menu tree, in Photoshop order; live where we implement the command.
    let known = is_live;
    let mut items: Vec<MenuItem> = crate::menu_catalog::CATALOG
        .iter()
        .map(|&(path, label, sc, id)| MenuItem {
            id: id.to_string(),
            label: label.to_string(),
            path: path.iter().map(|s| s.to_string()).collect(),
            // A live item shows the shortcut that runs it ([`crate::shortcut_dispatch::bindings`]
            // prefers the command's own over the catalogue's): Undo ⌘Z, Copy ⌘C, Hide Layers ⌘,.
            shortcut: if known(id) { crate::shortcuts::default_shortcut(id) } else { sc.map(Into::into) },
            enabled: known(id) && is_enabled(app, id),
            checked: checked(app, id),
            color: None,
        })
        .collect();
    // 2) Our commands that Photoshop's tree doesn't list (or lists under another id).
    let mut extra: Vec<MenuItem> = Vec::new();
    for &(id, label, path, sc) in UI_COMMANDS {
        extra.push(MenuItem {
            id: id.into(),
            label: label.into(),
            path: path.iter().map(|s| s.to_string()).collect(),
            shortcut: sc.map(Into::into),
            enabled: is_enabled(app, id),
            checked: checked(app, id),
            color: None,
        });
    }
    for c in photocraft_engine::command_specs().iter().filter(|c| !c.menu.is_empty()) {
        extra.push(MenuItem {
            id: c.id.into(),
            label: c.label.into(),
            path: c.menu.iter().map(|s| s.to_string()).collect(),
            shortcut: c.shortcut.map(Into::into),
            enabled: is_enabled(app, c.id),
            checked: None,
            color: None,
        });
    }
    for mut e in extra {
        let dup = items.iter().any(|i| i.id == e.id || (i.path == e.path && i.label.trim_end_matches('…') == e.label.trim_end_matches('…')));
        if !dup {
            // Insert after the item it belongs next to, else after the last item of the same
            // top-level menu, keeping menus contiguous.
            let top = e.path.first().cloned();
            let after = PLACE_AFTER.iter().find(|(id, _)| *id == e.id).and_then(|(_, a)| items.iter().position(|i| i.id == *a));
            if let Some(path) = after.and_then(|a| items.get(a)).map(|i| i.path.clone()) {
                e.path = path;
            }
            let at = after.or_else(|| items.iter().rposition(|i| i.path.first() == top.as_ref())).map_or(items.len(), |p| p + 1);
            items.insert(at, e);
        }
    }
    crate::plugin_ui::insert_menu_items(app, &mut items);
    if let Some(mask) = app.session.active().and_then(|s| s.active_layer.and_then(|id| s.doc.layer(id))).and_then(|l| l.mask.as_ref())
        && let Some(item) = items.iter_mut().find(|i| i.id == "layer.layerMask.enabled")
    {
        item.label = crate::layer_menu_ui::mask_toggle_label(mask.enabled).into();
    }
    // Layer › Merge Layers reads Merge Group with just a group selected, as in Photoshop (#3832).
    if let Some(st) = app.session.active()
        && st.selected_layers().len() <= 1
        && st.active_layer.and_then(|id| st.doc.layer(id)).is_some_and(|l| matches!(l.content, photocraft_doc::LayerContent::Group(_)))
        && let Some(item) = items.iter_mut().find(|i| i.id == "layer.mergeLayers")
    {
        item.label = "Merge Group".into();
    }
    // The File quick-export command uses the format selected in Export Preferences.
    // On the web it intentionally downloads PNG until the quick-export service supports
    // the other formats; the Layer quick-export command also always produces PNG.
    if let Some(item) = items.iter_mut().find(|i| i.id == "file.export.quickExportAsPng") {
        use photocraft_engine::prefs::QuickExportFormat;
        let format = if cfg!(target_arch = "wasm32") {
            "PNG"
        } else {
            match app.session.prefs().export.quick_export_format {
                QuickExportFormat::Png => "PNG",
                QuickExportFormat::Jpg => "JPG",
                QuickExportFormat::Gif => "GIF",
                QuickExportFormat::Webp => "WebP",
            }
        };
        item.label = format!("Quick Export as {format}");
    }
    // Smart Objects › Reveal in Finder names this platform's file manager (UI-217-19).
    if let Some(item) = items.iter_mut().find(|i| i.id == "layer.smartObjects.revealInFinder") {
        item.label = reveal_label().into();
    }
    // File › Open Recent: a dynamic submenu of recently opened files (inserted after "Open As…").
    if let Some(after) = items.iter().position(|i| i.id == "file.openAs") {
        let rp: Vec<String> = vec!["File".into(), "Open Recent".into()];
        let mut recent: Vec<MenuItem> = app
            .ui
            .recent_files
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let label = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                MenuItem { id: format!("file.openRecent.{i}"), label, path: rp.clone(), shortcut: None, enabled: true, checked: None, color: None }
            })
            .collect();
        if !recent.is_empty() {
            recent.push(MenuItem { id: "---".into(), label: "---".into(), path: rp.clone(), shortcut: None, enabled: false, checked: None, color: None });
        }
        recent.push(MenuItem {
            id: "file.clearRecent".into(),
            label: "Clear Recent Files".into(),
            path: rp.clone(),
            shortcut: None,
            enabled: !app.ui.recent_files.is_empty(),
            checked: None,
            color: None,
        });
        for (k, it) in recent.into_iter().enumerate() {
            items.insert(after + 1 + k, it);
        }
    }
    // Help: the link items, a separator, then System Info and About.
    if let Some(at) = items.iter().position(|i| i.id == "help.systemInfo" || i.id == "help.about") {
        items.insert(
            at,
            MenuItem { id: "---".into(), label: "---".into(), path: vec!["Help".into()], shortcut: None, enabled: false, checked: None, color: None },
        );
    }
    // Window: the open documents in tab order, after a separator, the active one checked.
    if let Some(last) = items.iter().rposition(|i| i.path.first().map(String::as_str) == Some("Window")) {
        let window = || vec!["Window".to_string()];
        let active = app.session.active_index();
        let docs = app.session.documents().iter().enumerate().map(|(i, st)| MenuItem {
            id: format!("{DOCUMENT_ITEM}{i}"),
            label: st.doc.name.clone(),
            path: window(),
            shortcut: None,
            enabled: true,
            checked: Some(active == Some(i)),
            color: None,
        });
        let separator = MenuItem { id: "---".into(), label: "---".into(), path: window(), shortcut: None, enabled: false, checked: None, color: None };
        let rows: Vec<MenuItem> = std::iter::once(separator).chain(docs).collect();
        if rows.len() > 1 {
            items.splice(last + 1..last + 1, rows);
        }
    }
    // Edit › Keyboard Shortcuts overrides, Edit › Menus hidden items and colours.
    let prefs = app.session.prefs();
    if !prefs.shortcuts.is_empty() {
        for it in &mut items {
            if prefs.shortcuts.contains_key(&it.id) {
                it.shortcut = prefs.shortcut(&it.id, None).map(str::to_string);
            }
        }
    }
    if !prefs.menus.hidden.is_empty() {
        items.retain(|i| !prefs.menus.is_hidden(&i.id));
    }
    if prefs.interface.show_menu_colors {
        for it in &mut items {
            it.color = prefs.menus.colors.get(&it.id).cloned();
        }
    }
    // Layer › Create Clipping Mask is Photoshop's one Create / Release item: when its shortcut
    // would release ([`crate::shortcut_dispatch::clip_key_releases`]) it reads Release Clipping
    // Mask and runs it, keeping that shortcut. The engine's own Release item lives in that slot.
    items.retain(|i| i.id != "layer.releaseClippingMask");
    if crate::shortcut_dispatch::clip_key_releases(app)
        && let Some(item) = items.iter_mut().find(|i| i.id == "layer.createClippingMask")
    {
        item.id = "layer.releaseClippingMask".into();
        item.label = "Release Clipping Mask".into();
        item.enabled = is_enabled(app, &item.id);
    }
    items
}

/// Background tint of an Edit › Menus colour name.
fn menu_tint(name: &str) -> Option<egui::Color32> {
    Some(match name {
        "red" => egui::Color32::from_rgb(190, 60, 60),
        "orange" => egui::Color32::from_rgb(200, 120, 40),
        "yellow" => egui::Color32::from_rgb(190, 170, 40),
        "green" => egui::Color32::from_rgb(60, 150, 70),
        "blue" => egui::Color32::from_rgb(50, 110, 200),
        "violet" => egui::Color32::from_rgb(130, 80, 190),
        "gray" => egui::Color32::from_rgb(120, 120, 120),
        _ => return None,
    })
}

/// The open menus' items, kept between frames: building them asks every command whether it can
/// run, which on a document with thousands of layers costs far more than a frame.
pub(crate) struct ItemCache {
    /// [`crate::native_menu::state_hash`] of the state the items were built from.
    state: u64,
    items: Vec<MenuItem>,
}

impl ItemCache {
    /// The items, while they still describe `state`. A frame with a click or key press rebuilds
    /// them, which covers state the hash doesn't list.
    fn reuse(self, state: u64, input: bool) -> Option<Vec<MenuItem>> {
        (!input && self.state == state).then_some(self.items)
    }
}

/// Horizontal room the expanded File…Help titles need (label widths + button padding).
pub fn menu_bar_needed_width(ui: &egui::Ui) -> f32 {
    let lang = crate::i18n::current();
    let font = egui::TextStyle::Button.resolve(ui.style());
    // Matches `button_padding = (6, 3)` used while drawing the titles.
    const PAD_X: f32 = 12.0;
    TOP_MENUS
        .iter()
        .map(|&top| {
            let label = crate::i18n::tr(lang, top).to_owned();
            let text_w = ui.painter().layout_no_wrap(label, font.clone(), egui::Color32::WHITE).size().x;
            text_w + PAD_X
        })
        .sum()
}

/// Hit area of the collapsed hamburger control in the menu bar.
pub const MENU_BAR_HAMBURGER_WIDTH: f32 = 28.0;

/// True when the expanded titles need more than `budget` points of room.
pub fn menu_bar_collapses(needed: f32, budget: f32) -> bool {
    needed > budget.max(0.0)
}

/// Draws the menu bar; returns the inner edge of the menu titles, facing the rest of the title bar
/// (the right edge of the last title, or its left edge in a right-to-left UI, where File is at the
/// right). The bar itself fills the row.
pub fn menu_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, budget: f32) -> f32 {
    // `budget` is how much horizontal room the titles may take. When the expanded File…Help strip
    // needs more than that, the titles fold into a hamburger (`align-justify` SVG) whose popup lists
    // every top-level menu as a submenu.

    // Built only while a menu is open: every item's enabled/checked state scales with the
    // document (layer lookups), which cost milliseconds per frame on large layouts (#125). While
    // one stays open, the items are reused until what they depend on changes.
    let kept = std::cell::Cell::new(app.menu_cache.take());
    let built: std::cell::OnceCell<ItemCache> = std::cell::OnceCell::new();
    let app_ref: &PhotocraftApp = app;
    let rtl = crate::direction::is_rtl();
    let mut edge = if rtl { ui.cursor().right() } else { ui.cursor().left() };
    let lang = crate::i18n::current();
    let mut clicked: Option<String> = None;
    let t = crate::theme::Tokens::get(ui.ctx());
    let mut nav = crate::menu_nav::Nav::load(ui.ctx());
    // The keyboard on the bar (#4988): Alt / F10 focus, Alt+letter, mnemonics.
    let mut keys = crate::menu_keys::Keys::load(ui.ctx());
    keys.take_first_row(&mut nav);
    nav.underline = keys.show_underlines(ui.ctx());
    let top_labels: Vec<String> = TOP_MENUS.iter().map(|top| crate::i18n::tr(lang, top).to_string()).collect();
    let preferred: Vec<Option<char>> = TOP_MENUS.iter().map(|top| crate::menu_keys::TOP_PREFERRED.iter().find(|(t, _)| t == top).map(|(_, c)| *c)).collect();
    let top_mnemonics = crate::menu_keys::assign(&top_labels.iter().map(String::as_str).collect::<Vec<_>>(), &preferred);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let v = &mut ui.style_mut().visuals;
        v.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, t.text_dim);
        v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        egui::MenuBar::new().ui(ui, |ui| {
            // egui lays the bar out left to right; a right-to-left UI starts it at the right.
            let mut mirrored = rtl.then(|| ui.new_child(egui::UiBuilder::new().layout(crate::direction::row())));
            let ui = mirrored.as_mut().unwrap_or(ui);
            ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
            nav.bar_bottom = Some(ui.max_rect().bottom());
            let collapse = menu_bar_collapses(menu_bar_needed_width(ui), budget);
            let mut buttons = Vec::with_capacity(if collapse { 1 } else { TOP_MENUS.len() });
            if collapse {
                // Three-line (align-justify) SVG: opens a secondary menu of File…Help.
                let tip = crate::i18n::tr(lang, "Menus").to_owned();
                let title = ui.add(
                    egui::Button::image(crate::icons::image("align-justify", 14.0, t.text_dim))
                        .frame(false)
                        .min_size(egui::vec2(MENU_BAR_HAMBURGER_WIDTH, ui.spacing().interact_size.y.max(24.0))),
                );
                let open_now = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&title));
                if title.hovered() || open_now {
                    crate::icons::paint(ui, title.rect, "align-justify", 14.0, t.text);
                }
                let title = title.on_hover_text(tip.clone());
                title.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip.clone()));
                let bar = egui::containers::menu::MenuConfig::find(ui);
                let config = egui::containers::menu::MenuConfig::new().close_behavior(bar.close_behavior).style(bar.style.clone());
                let open = title.clicked().then_some(egui::SetOpenCommand::Toggle);
                let opening = title.clicked();
                let close = if opening { egui::PopupCloseBehavior::IgnoreClicks } else { egui::PopupCloseBehavior::CloseOnClickOutside };
                egui::Popup::menu(&title)
                    .align(crate::direction::below())
                    .open_memory(open)
                    .close_behavior(close)
                    .style(config.style.clone())
                    .info(egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(egui::containers::menu::MenuConfig::MENU_CONFIG_TAG, config))
                    .show(|ui| {
                        let items = &built
                            .get_or_init(|| {
                                let state = crate::native_menu::state_hash(app_ref, None);
                                let input = crate::native_menu::had_input(ui.ctx());
                                let items = kept.take().and_then(|c| c.reuse(state, input)).unwrap_or_else(|| menu_items(app_ref));
                                ItemCache { state, items }
                            })
                            .items;
                        ui.set_min_width(180.0);
                        crate::menu_nav::level(ui, 1, &mut nav, |ui, nav| {
                            for top in TOP_MENUS {
                                let mine: Vec<&MenuItem> = items.iter().filter(|i| i.path.first().map(String::as_str) == Some(top)).collect();
                                let enabled = !mine.is_empty();
                                ui.add_enabled_ui(enabled, |ui| {
                                    nav.row(ui, 0, enabled, None, |ui, nav| {
                                        let label = crate::i18n::tr(lang, top);
                                        let r = ui.menu_button(label, |ui| {
                                            ui.set_min_width(220.0);
                                            if mine.is_empty() {
                                                ui.weak(crate::i18n::tr(lang, "(coming soon)"));
                                            }
                                            if top == "Help" {
                                                crate::menu_nav::level(ui, 1, nav, |ui, nav| {
                                                    help_search(ui, items, opening, &mut clicked, nav);
                                                    render_level_rows(ui, &mine, 1, &mut clicked, nav);
                                                });
                                            } else {
                                                render_level(ui, &mine, 1, &mut clicked, nav);
                                            }
                                        });
                                        if r.inner.is_some() {
                                            nav.set_anchor(1, r.response.rect);
                                        }
                                        (r.response, ())
                                    });
                                });
                            }
                        });
