//! HarmonyOS-only FFI boundary. The editor and all algorithms remain safe Rust.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
mod bridge;
#[cfg(any(target_env = "ohos", test))]
mod chrome;
#[cfg(any(target_env = "ohos", test))]
mod cursor;
#[cfg(any(target_env = "ohos", test))]
mod display_color;
#[cfg(any(target_env = "ohos", test))]
mod documents;
#[cfg(any(target_env = "ohos", test))]
mod ime;
mod input;
#[cfg(target_env = "ohos")]
mod render;
#[cfg(any(target_env = "ohos", test))]
mod working_directory;
#[cfg(target_env = "ohos")]
use photocraft_ui_egui::PhotocraftApp;
#[cfg(target_env = "ohos")]
use photocraft_ui_egui::Services;
use serde::Deserialize;
use std::{
    sync::{OnceLock, mpsc},
    time::Duration,
};

#[cfg(target_env = "ohos")]
use std::time::Instant;

#[derive(Deserialize)]
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
struct Config {
    files: String,
    #[serde(default)]
    documents: String,
    #[serde(default = "default_scale")]
    scale: f32,
    #[serde(default)]
    language: String,
    #[serde(default, rename = "displayP3")]
    display_p3: bool,
}
fn default_scale() -> f32 {
    1.
}
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
enum Message {
    Attach(usize, u32, u32),
    Resize(u32, u32),
    Detach,
    Frame,
    Active(bool),
    Input(input::Input),
    Open(String, Vec<u8>),
    Close,
    Wake(Duration),
    Command(String, serde_json::Value),
}
static SEND: OnceLock<mpsc::Sender<Message>> = OnceLock::new();
fn send(message: Message) {
    if let Some(tx) = SEND.get() {
        let _ = tx.send(message);
    }
}
#[cfg(target_env = "ohos")]
fn services(
    files: &str,
    documents: std::sync::Arc<std::sync::Mutex<documents::Documents>>,
) -> Services {
    let load = std::path::Path::new(files).join("native-preferences.json");
    let save = load.clone();
    Services {
        import: Some(Box::new(move |name, bytes| {
            photocraft_io::import(name, bytes)
                .map(|r| (r.document, r.warnings))
                .map_err(|e| e.to_string())
        })),
        export: Some(Box::new(|doc, name, settings| {
            let mut options = photocraft_io::ExportOptions::default();
            if let Some(q) = settings.jpeg_quality {
                options.encode.jpeg_quality = q;
            }
            options.encode.webp_lossless = settings.webp_lossless;
            if let Some(q) = settings.webp_quality {
                options.encode.webp_quality = q;
            }
            photocraft_io::export(doc, name, &options)
                .map(|r| (r.bytes, r.warnings))
                .map_err(|e| e.to_string())
        })),
        pick_open: Some(Box::new(move || match bridge::request("open", "", &[]) {
            Ok((name, bytes)) => Some((name, Ok(bytes))),
            Err(e) if e == "cancelled" => None,
            Err(e) => Some(("file".into(), Err(e))),
        })),
        always_pick_save: true,
        default_save: Some(Box::new(move |doc, source| {
            if let Some(path) = documents
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .existing(doc.id.0, source)
            {
                return Ok(path);
            }
            let stem = doc
                .name
                .rsplit_once('.')
                .map_or(doc.name.as_str(), |(stem, _)| stem);
            let (name, _) = bridge::request("save_default", &format!("{stem}.psd"), &[])?;
            documents
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .select(doc.id.0, &name)
        })),
        pick_save: Some(Box::new(|suggested| {
            match bridge::request("save_select", suggested, &[]) {
                Ok((name, _)) => Some(name),
                Err(e) => {
                    if e != "cancelled" {
                        bridge::notify(0, "error", &e, &[]);
                    }
                    None
                }
            }
        })),
        write: Some(Box::new(move |name, bytes| {
            bridge::request("write", name, bytes).map(|_| ())
        })),
        open_url: Some(Box::new(|url| {
            bridge::notify(0, "url", url, &[]);
            Ok(())
        })),
        load_prefs: Some(Box::new(move || std::fs::read_to_string(&load).ok())),
        save_prefs: Some(Box::new(move |text| {
            photocraft_format::atomic_write(&save, text.as_bytes()).map_err(|e| e.to_string())
        })),
        ..Default::default()
    }
}
// SAFETY: C++ passes a valid pointer/length pair for the duration of each call. Copy
// immediately; never retain a borrowed C++ buffer. Reject allocation-sized hostile inputs.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len > 256 * 1024 * 1024 || (ptr.is_null() && len != 0) {
        return None;
    }
    if len == 0 {
        return Some(&[]);
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}
/// # Safety
/// Pointer/length pairs must reference valid, readable memory until this call returns.
/// The notification function supplied to pc_init must remain valid for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_init(ptr: *const u8, len: usize, notify: bridge::Notify) -> bool {
    let Some(data) = (unsafe { bytes(ptr, len) }) else {
        return false;
    };
    let Ok(config) = serde_json::from_slice::<Config>(data) else {
        return false;
    };
    if !config.scale.is_finite() || config.scale <= 0. || config.scale > 8. {
        return false;
    }
    bridge::init(notify);
    let (tx, rx) = mpsc::channel();
    if SEND.set(tx).is_err() {
        return true;
    }
    std::thread::Builder::new()
        .name("photocraft-native".into())
        .spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker(config, rx)))
                .is_err()
            {
                bridge::notify(0, "error", "Native host initialization failed", &[]);
            }
        })
        .is_ok()
}
/// # Safety
/// Pointer/length pairs must reference valid, readable memory until this call returns.
/// The notification function supplied to pc_init must remain valid for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_input(ptr: *const u8, len: usize) {
    if let Some(data) = unsafe { bytes(ptr, len) }
        && let Ok(value) = serde_json::from_slice(data)
    {
        send(Message::Input(value));
    }
}
/// # Safety
/// Pointer/length pairs must reference valid, readable memory until this call returns.
/// The notification function supplied to pc_init must remain valid for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_reply(
    id: u64,
    ok: bool,
    text: *const u8,
    text_len: usize,
    data: *const u8,
    len: usize,
) {
    let Some(text) = (unsafe { bytes(text, text_len) }) else {
        return;
    };
    let Some(data) = (unsafe { bytes(data, len) }) else {
        return;
    };
    let text = String::from_utf8_lossy(text).into_owned();
    bridge::reply(
        id,
        if ok {
            Ok((text, data.to_vec()))
        } else {
            Err(text)
        },
    );
}
/// # Safety
/// Pointer/length pairs must reference valid, readable memory until this call returns.
/// The notification function supplied to pc_init must remain valid for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_open(text: *const u8, n: usize, data: *const u8, len: usize) {
    if let (Some(text), Some(data)) = (unsafe { bytes(text, n) }, unsafe { bytes(data, len) }) {
        send(Message::Open(
            String::from_utf8_lossy(text).into_owned(),
            data.to_vec(),
        ));
    }
}
/// # Safety
/// Pointer/length pairs must reference valid, readable memory until this call returns.
/// The notification function supplied to pc_init must remain valid for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_command(ptr: *const u8, len: usize) {
    if let Some(data) = unsafe { bytes(ptr, len) }
        && let Ok(v) = serde_json::from_slice::<serde_json::Value>(data)
        && let Some(id) = v.get("id").and_then(|v| v.as_str())
    {
        send(Message::Command(
            id.into(),
            v.get("params").cloned().unwrap_or_default(),
        ));
    }
}
/// # Safety
/// `window` must be a live OHNativeWindow with one acquired native reference.
/// Ownership of that reference transfers to the worker and its wgpu surfaces;
/// the caller must not release that reference after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_attach(window: usize, w: u32, h: u32) {
    if window != 0 {
        send(Message::Attach(window, w, h));
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_resize(w: u32, h: u32) {
    send(Message::Resize(w, h));
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_detach() {
    bridge::cancel();
    send(Message::Detach);
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_frame() {
    send(Message::Frame);
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_active(active: bool) {
    send(Message::Active(active));
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_close() {
    send(Message::Close);
}

#[cfg(target_env = "ohos")]
unsafe extern "C" {
    fn pc_request_vsync();
}
#[cfg(target_env = "ohos")]
fn worker(config: Config, rx: mpsc::Receiver<Message>) {
    bridge::notify(0, "stage", "Rust worker entered", &[]);
    let working_directory = match working_directory::initialize(&config.files) {
        Ok(path) => format!("Current working directory: {}", path.display()),
        Err(error) => {
            bridge::notify(0, "error", &error, &[]);
            return;
        }
    };
    bridge::notify(0, "diagnostic", &working_directory, &[]);
    photocraft_ui_egui::i18n::set_host_locale(&config.language);
    photocraft_text::cjk::set_ui_locale(Some(&config.language));
    let document_files = if config.documents.is_empty() {
        &config.files
    } else {
        &config.documents
    };
    let documents = std::sync::Arc::new(std::sync::Mutex::new(documents::Documents::new(
        document_files,
    )));
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    bridge::notify(0, "stage", "egui context ready", &[]);
    let mut app = PhotocraftApp::new(
        photocraft_engine::Session::new(),
        services(&config.files, documents.clone()),
    );
    app.session.print_service = Some(|pdf, metadata| {
        bridge::request("print", &metadata.to_string(), pdf).map(|(message, _)| message)
    });
    app.session.print_save_service =
        Some(|target, pdf| bridge::request("write", target, pdf).map(|_| ()));

    // Apply the app's initial style/font configuration before the host registers
    // system fonts. This startup pass has no user input.
    let _startup = ctx.run_logic(&egui::RawInput::default(), |ctx| app.host_logic(ctx));

    bridge::notify(0, "stage", "editor ready", &[]);
    // Host CJK registration inspects the font definitions. egui makes them
    // available only after a first UI pass. Preserve that pass's texture deltas
    // for the first actual presentation; no font atlas upload may be skipped.
    let mut initial_textures = initialize_font_context(&ctx);
    photocraft_ui_egui::i18n::set_current(photocraft_ui_egui::i18n::Lang::from_pref(
        &app.session.prefs().interface.language,
    ));
    for (script, name) in [
        ("zh-hans", "HarmonyOS_Sans_SC.ttf"),
        ("zh-hant", "HarmonyOS_Sans_TC.ttf"),
    ] {
        if let Ok(data) = std::fs::read(format!("/system/fonts/{name}")) {
            photocraft_ui_egui::cjk_fonts::store_host_cjk_font(script, data);
        }
    }
    photocraft_ui_egui::cjk_fonts::reapply_host_fonts(&ctx);
    bridge::notify(0, "stage", "fonts ready", &[]);
    ctx.set_request_repaint_callback(|info| send(Message::Wake(info.delay)));
    app.perf.gpu_info.canvas = "cpu".into();
    app.perf.gpu_info.selected = "cpu".into();
    let mut render: Option<render::Render> = None;
    let mut active = true;
    let mut visible = false;
    let mut deadline: Option<Instant> = None;
    let started = Instant::now();
    let mut events = vec![];
    let mut input = input::State::default();
    input.feed = app.stylus.feed.clone();
    let mut ime = ime::ImeSync::default();
    let mut close = false;
    let mut chrome = chrome::ChromeSync::default();
    let mut titlebar_pixels = egui::Vec2::ZERO;
    // 输入、窗口生命周期和帧消息共用一个队列，编辑器状态仅在 worker 上修改。
    let mut cursor = cursor::CursorSync::default();
    loop {
        let wait = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        let message = match wait {
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(m) => m,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    deadline = None;
                    if active && visible && render.is_some() {
                        unsafe {
                            pc_request_vsync();
                        }
                    }
                    continue;
                }
                Err(_) => break,
            },
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => break,
            },
        };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                match message {
                    Message::Attach(ptr, w, h) => {
                        chrome.invalidate();
                        cursor.invalidate();
                        bridge::notify(0, "stage", "creating native GPU surface", &[]);
                        if let Some(r) = render.as_mut() {
                            r.bind(render::Window(ptr), w, h)?;
                        } else {
                            let r =
                                render::Render::new(render::Window(ptr), w, h, config.display_p3)?;
                            r.color.install(&mut app.session.color);
                            app.perf.gpu_info.set_adapter(&r.adapter.get_info());
                            render = Some(r);
                        }
                        visible = w > 0 && h > 0;
                        bridge::resume();
                        deadline = Some(Instant::now());
                        bridge::notify(0, "ready", "Native egui / wgpu GLES", &[]);
                    }
                    Message::Resize(w, h) => {
                        visible = w > 0 && h > 0;
                        if let Some(r) = render.as_mut() {
                            r.resize(w, h)?;
                        }
                        deadline = Some(Instant::now());
                    }
                    Message::Detach => {
                        ime.suspend();
                        if let Some(r) = render.as_mut() {
                            r.detach();
                        }
                        visible = false;
                        deadline = None;
                        input.cancel(egui::Pos2::ZERO, &mut events);
                    }
                    Message::Active(value) => {
                        active = value;
                        if active {
                            chrome.invalidate();
                            cursor.invalidate();
                            bridge::resume();
                            deadline = Some(Instant::now());
                        } else {
                            ime.suspend();
                            deadline = None;
                            input.cancel(egui::Pos2::ZERO, &mut events);
                        }
                    }
                    Message::Input(value) => {
                        if value.kind == "titlebar" {
                            titlebar_pixels = egui::vec2(value.dx, value.dy);
                        }
                        if value.kind == "blur" {
                            cursor.invalidate();
                        }
                        if value.kind.starts_with("ime_") {
                            ime.input(&ctx, Some(&mut app), value, &mut events);
                        } else {
                            input.push(value, ctx.pixels_per_point(), &mut events);
                        }
                        deadline = Some(Instant::now());
                    }
                    Message::Open(name, data) => {
                        app.open_bytes(&name, &data)?;
                        deadline = Some(Instant::now());
                    }
                    Message::Close => {
                        bridge::resume();
                        close = true;
                        deadline = Some(Instant::now());
                    }
                    Message::Wake(delay) => {
                        if active
                            && visible
                            && let Some(next) = Instant::now().checked_add(delay)
                        {
                            deadline = Some(deadline.map_or(next, |d| d.min(next)));
                        }
                    }
                    Message::Command(id, params) => {
                        let result = photocraft_ui_egui::menus::invoke(&mut app, &ctx, &id, params);
                        bridge::notify(0, "command", &format!("{id}: {result:?}"), &[]);
                        deadline = Some(Instant::now());
                    }
                    Message::Frame => {
                        if !active || !visible {
                            return Ok(());
                        }
                        let Some(r) = render.as_mut() else {
                            return Ok(());
                        };
                        let size = egui::vec2(
                            r.config.width as f32 / (config.scale * ctx.zoom_factor()),
                            r.config.height as f32 / (config.scale * ctx.zoom_factor()),
                        );
                        let mut raw = egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            time: Some(started.elapsed().as_secs_f64()),
                            events: std::mem::take(&mut events),
                            ..Default::default()
                        };
                        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
                        viewport.native_pixels_per_point = Some(config.scale);
                        viewport.inner_rect = raw.screen_rect;
                        viewport.focused = Some(true);
                        let validating_close = close;
                        if close {
                            viewport.events.push(egui::ViewportEvent::Close);
                            close = false;
                        }
                        app.host_input(&ctx, &mut raw);
                        // 保存物理像素而非换算结果，确保界面缩放后仍正确避让系统三键。
                        app.host_titlebar = chrome::titlebar_geometry(
                            titlebar_pixels.x,
                            titlebar_pixels.y,
                            config.scale * ctx.zoom_factor(),
                        );
                        ime.pointer_input(&raw.events);
                        let mut output = editor_frame(&ctx, &mut app, raw);
                        // Preferences and menu actions have applied the theme by this point.
                        if let Some(color) = chrome.update(app.ui.theme) {
                            bridge::notify(0, "chrome", &color, &[]);
                        }
                        if !initial_textures.is_empty() {
                            initial_textures.append(std::mem::take(&mut output.textures_delta));
                            output.textures_delta = std::mem::take(&mut initial_textures);
                        }
                        if let Some(icon) = cursor.update(output.platform_output.cursor_icon) {
                            bridge::notify(0, "cursor", &icon, &[]);
                        }
                        if ime::reveal_canvas_caret(&mut app, &output.platform_output) {
                            ctx.request_repaint();
                        }
                        if let Some(state) = ime.update(&ctx, &output.platform_output, Some(&app)) {
                            bridge::notify(0, "ime", &state, &[]);
                        }
                        let mut commands = Vec::new();
                        if let Some(v) = output.viewport_output.get(&egui::ViewportId::ROOT) {
                            commands.extend(v.commands.clone());
                        }
                        match close_action(validating_close, &commands) {
                            CloseAction::Confirm => bridge::notify(0, "close", "", &[]),
                            CloseAction::Request => {
                                // egui itself emits Close for Ctrl+Q before the app's
                                // shortcut handler runs. Return it as a viewport event
                                // next frame so the editor can veto unsaved work.
                                close = true;
                                deadline = Some(Instant::now());
                            }
                            CloseAction::Cancel | CloseAction::None => {}
                        }
                        for command in &output.platform_output.commands {
                            if let egui::OutputCommand::OpenUrl(url) = command {
                                bridge::notify(0, "url", &url.url, &[]);
                            }
                        }
                        let delay = output
                            .viewport_output
                            .get(&egui::ViewportId::ROOT)
                            .map(|v| v.repaint_delay)
                            .unwrap_or(Duration::MAX);
                        if !r.paint(&ctx, output)? {
                            deadline = Some(Instant::now() + Duration::from_millis(50));
                        } else if let Some(next) = Instant::now().checked_add(delay) {
                            deadline = Some(deadline.map_or(next, |d| d.min(next)));
                        }
                    }
                }
                Ok(())
            }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                app.ui.status = e.clone();
                app.ui.status_error = true;
                bridge::notify(0, "error", &e, &[]);
            }
            Err(_) => {
                bridge::notify(
                    0,
                    "error",
                    "Native rendering failed; document remains in memory",
                    &[],
                );
                render = None;
                deadline = None;
            }
        }
    }
}
#[cfg(not(target_env = "ohos"))]
fn worker(_config: Config, _rx: mpsc::Receiver<Message>) {
    bridge::notify(0, "error", "Native host requires an OHOS target", &[]);
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
enum CloseAction {
    None,
    Request,
    Cancel,
    Confirm,
}
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
fn close_action(validating: bool, commands: &[egui::ViewportCommand]) -> CloseAction {
    if commands.contains(&egui::ViewportCommand::CancelClose) {
        CloseAction::Cancel
    } else if validating {
        CloseAction::Confirm
    } else if commands.contains(&egui::ViewportCommand::Close) {
        CloseAction::Request
    } else {
        CloseAction::None
    }
}

// A custom host must initialize egui's font state before a platform font loader
// inspects it. Return the atlas deltas so the first displayed frame uploads them.
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
fn initialize_font_context(ctx: &egui::Context) -> egui::TexturesDelta {
    let mut warm = ctx.run_ui(egui::RawInput::default(), |_| {});
    let textures = std::mem::take(&mut warm.textures_delta);
    warm.drop_without_applying_deltas();
    textures
}

/// run_logic only updates viewport metadata; it does not ingest this frame's
/// keyboard input. Run editor logic after begin_pass, once even if egui requests
/// another layout pass, so shortcuts see the new keys and commands are not repeated.
#[cfg(any(target_env = "ohos", test))]
fn editor_frame(
    ctx: &egui::Context,
    app: &mut photocraft_ui_egui::PhotocraftApp,
    raw: egui::RawInput,
) -> egui::FullOutput {
    let mut first_pass = true;
    ctx.run_ui(raw, |ui| {
        if first_pass {
            first_pass = false;
            app.host_logic(ui.ctx());
        }
        app.host_ui(ui);
    })
}
#[cfg(test)]
#[path = "../tests/unit/host.rs"]
mod host_tests;
