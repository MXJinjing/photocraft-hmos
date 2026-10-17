//! HarmonyOS-only FFI boundary. The editor and all algorithms remain safe Rust.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#[cfg(any(target_env = "ohos", test))]
mod appearance;
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
mod drop_import;
#[cfg(feature = "host-recovery-test")]
mod fault_injection;
mod file_open;
#[cfg(any(target_env = "ohos", test))]
mod ime;
mod input;
#[cfg(any(target_env = "ohos", test))]
mod recovery;
#[cfg(any(target_env = "ohos", test))]
mod viewports;
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
    #[serde(default, rename = "systemTheme")]
    system_theme: String,
}
fn default_scale() -> f32 {
    1.
}
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
enum Message {
    Attach(usize, u32, u32),
    Resize(u32, u32),
    Detach,
    /// Native sub-window (floating document window) surface lifecycle, by window key.
    WinAttach(u32, usize, u32, u32),
    WinResize(u32, u32, u32),
    WinDetach(u32),
    Frame,
    Active(bool),
    Input(input::Input),
    Open(String, Vec<u8>),
    Drop(String, Vec<u8>, f32, f32),
    DropFile(String, std::fs::File, f32, f32),
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
    appearance: &appearance::Appearance,
) -> Services {
    let load = std::path::Path::new(files).join("native-preferences.json");
    let save = load.clone();
    Services {
        import: Some(Box::new(move |name, bytes, max_svg_group_depth| {
            photocraft_io::import_with_svg_group_depth(name, bytes, max_svg_group_depth)
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
        load_prefs: Some(Box::new(move || {
            std::fs::read_to_string(&load).ok().or_else(|| {
                // First launch on HarmonyOS: follow the system colour scheme.
                Some(r#"{"interface":{"appearanceMode":"auto"}}"#.into())
            })
        })),
        save_prefs: Some(Box::new(move |text| {
            photocraft_format::atomic_write(&save, text.as_bytes()).map_err(|e| e.to_string())
        })),
        system_theme: Some(appearance.service()),
        ..Default::default()
    }
}
// SAFETY: C++ passes a valid pointer/length pair for the duration of each call. Copy
// immediately; never retain a borrowed C++ buffer. Enforce slice addressability.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len > isize::MAX as usize || (ptr.is_null() && len != 0) {
        return None;
    }
    if len == 0 {
        return Some(&[]);
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}
fn copy_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(data.len())
        .map_err(|e| format!("Not enough memory to open file: {e}"))?;
    bytes.extend_from_slice(data);
    Ok(bytes)
}

fn finish_file(id: Option<u64>, result: bridge::Reply) {
    if let Some(id) = id {
        bridge::reply(id, result);
    } else {
        match result {
            Ok((name, data)) => send(Message::Open(name, data)),
            Err(error) => bridge::notify(0, "error", &error, &[]),
        }
    }
}

// SAFETY: The platform holds the original file open until the synchronous duplicate returns.
unsafe fn open_file(id: Option<u64>, name: String, fd: i32) {
    let file = match unsafe { file_open::duplicate(fd) } {
        Ok(file) => file,
        Err(error) => {
            finish_file(id, Err(error));
            return;
        }
    };
    // Do not read on the render worker: it can be waiting for this picker reply.
    if let Err(error) = std::thread::Builder::new()
        .name("photocraft-file-open".into())
        .spawn(move || finish_file(id, file_open::read(file).map(|data| (name, data))))
    {
        finish_file(id, Err(format!("Cannot start file reader: {error}")));
    }
}

/// # Safety
/// `text`/`n` must be readable and `fd` open until this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_reply_file(id: u64, text: *const u8, n: usize, fd: i32) {
    let Some(text) = (unsafe { bytes(text, n) }) else {
        bridge::reply(id, Err("Invalid file name buffer".into()));
        return;
    };
    unsafe { open_file(Some(id), String::from_utf8_lossy(text).into_owned(), fd) };
}

/// # Safety
/// `text`/`n` must be readable and `fd` open until this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_open_file(text: *const u8, n: usize, fd: i32) {
    let Some(text) = (unsafe { bytes(text, n) }) else {
        bridge::notify(0, "error", "Invalid file name buffer", &[]);
        return;
    };
    unsafe { open_file(None, String::from_utf8_lossy(text).into_owned(), fd) };
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
        bridge::reply(id, Err("Invalid reply text buffer".into()));
        return;
    };
    let Some(data) = (unsafe { bytes(data, len) }) else {
        bridge::reply(id, Err("Invalid reply data buffer".into()));
        return;
    };
    let text = String::from_utf8_lossy(text).into_owned();
    bridge::reply(
        id,
        if ok {
            copy_bytes(data).map(|data| (text, data))
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
        match copy_bytes(data) {
            Ok(data) => send(Message::Open(
                String::from_utf8_lossy(text).into_owned(),
                data,
            )),
            Err(error) => bridge::notify(0, "error", &error, &[]),
        }
    }
}
/// # Safety
/// Pointer/length pairs must reference valid, readable memory until this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_drop(
    text: *const u8,
    n: usize,
    data: *const u8,
    len: usize,
    x: f32,
    y: f32,
) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    if let (Some(text), Some(data)) = (unsafe { bytes(text, n) }, unsafe { bytes(data, len) }) {
        send(Message::Drop(
            String::from_utf8_lossy(text).into_owned(),
            data.to_vec(),
            x,
            y,
        ));
    }
}
/// # Safety
/// `text`/`n` must be readable and `fd` open until this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_drop_file(text: *const u8, n: usize, fd: i32, x: f32, y: f32) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let Some(text) = (unsafe { bytes(text, n) }) else {
        return;
    };
    // SAFETY: ArkTS holds its descriptor open during this call; only our duplicate is queued.
    match unsafe { file_open::duplicate(fd) } {
        Ok(file) => send(Message::DropFile(
            String::from_utf8_lossy(text).into_owned(),
            file,
            x,
            y,
        )),
        Err(error) => bridge::notify(0, "error", &error, &[]),
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
/// # Safety
/// Same contract as [`pc_attach`], for the native sub-window with the given key (> 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_win_attach(key: u32, window: usize, w: u32, h: u32) {
    if window != 0 && key != 0 {
        send(Message::WinAttach(key, window, w, h));
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_win_resize(key: u32, w: u32, h: u32) {
    send(Message::WinResize(key, w, h));
}
#[unsafe(no_mangle)]
pub extern "C" fn pc_win_detach(key: u32) {
    send(Message::WinDetach(key));
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
    let appearance = appearance::Appearance::from_token(&config.system_theme);
    let mut app = PhotocraftApp::new(
        photocraft_engine::Session::new(),
        services(&config.files, documents.clone(), &appearance),
    );
    // Native file opens and drag imports must decode off the render worker so the
    // editor keeps drawing the opening tab and its progress indicator.
    app.background_jobs = true;
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
    let mut render = match render::Render::new(config.display_p3) {
        Ok(render) => render,
        Err(error) => {
            bridge::notify(0, "error", &error, &[]);
            return;
        }
    };
    // Floating document windows are native HarmonyOS sub-windows, each with its own surface.
    let registry = std::rc::Rc::new(std::cell::RefCell::new(viewports::Registry::new(
        config.scale,
        app.stylus.feed.clone(),
    )));
    viewports::install(registry.clone());
    app.native_doc_windows = true;
    let ui_pass_in_progress = std::cell::Cell::new(false);
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
        // Repaint callbacks and input must not bypass a failed producer's backoff.
        deadline = deadline.map(|next| render.recovery.frame_deadline(next));
        let wait = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        let message = match wait {
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(m) => m,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    deadline = None;
                    if active && visible && render.attached() {
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
        let rendering_operation = matches!(
            &message,
            Message::Attach(..) | Message::Resize(..) | Message::Detach | Message::Frame
        );
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                match message {
                    Message::Attach(ptr, w, h) => {
                        if render.recovery.halted {
                            return Ok(());
                        }
                        chrome.invalidate();
                        cursor.invalidate();
                        bridge::notify(0, "stage", "creating native GPU surface", &[]);
                        visible = w > 0 && h > 0;
                        let initialize_gpu = render.egui_render_state().is_none();
                        render.bind(render::Window(ptr), w, h)?;
                        render.color().install(&mut app.session.color);
                        if initialize_gpu && let Some(rs) = render.egui_render_state() {
                            app.perf.gpu_info.set_adapter(&rs.adapter.get_info());
                            app.perf.gpu_info.selected = "gl".into();
                            if rs.adapter.get_info().device_type == wgpu::DeviceType::Cpu {
                                return Err(
                                    "No hardware GPU adapter; drawing stopped; restart PhotoCraft"
                                        .into(),
                                );
                            }
                            app.set_wgpu(rs);
                            render.observe_canvas(&app);
                            if !app.gpu_active() {
                                return Err(app
                                    .perf
                                    .gpu_fallback
                                    .clone()
                                    .unwrap_or_else(|| "GPU canvas initialization failed".into()));
                            }
                        }
                        bridge::resume();
                        deadline = Some(Instant::now());
                        bridge::notify(0, "ready", "Native egui / wgpu GLES", &[]);
                    }
                    Message::Resize(w, h) => {
                        if render.recovery.halted {
                            return Ok(());
                        }
                        visible = w > 0 && h > 0;
                        render.resize(w, h)?;
                        deadline = Some(Instant::now());
                    }
                    Message::Detach => {
                        ime.suspend();
                        visible = false;
                        deadline = None;
                        render.detach();
                        input.cancel(egui::Pos2::ZERO, &mut events);
                    }
                    Message::WinAttach(key, ptr, w, h) => {
                        // The native reference is owned from here on; dropping it unrefs.
                        let window = render::Window(ptr);
                        if render.recovery.halted || !registry.borrow().knows(key) {
                            return Ok(());
                        }
                        let attached = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || render.attach_child(key, window, w, h),
                        ))
                        .unwrap_or_else(|_| Err("native sub-window surface panicked".into()));
                        match attached {
                            Ok(()) => registry.borrow_mut().attach(key, w, h),
                            Err(error) => {
                                bridge::notify(
                                    0,
                                    "diagnostic",
                                    &format!("Native sub-window {key} unavailable: {error}"),
                                    &[],
                                );
                                render.remove_child(key);
                                registry.borrow_mut().request_close(key);
                            }
                        }
                        deadline = Some(Instant::now());
                    }
                    Message::WinResize(key, w, h) => {
                        if render.recovery.halted {
                            return Ok(());
                        }
                        let resized = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || render.resize_child(key, w, h),
                        ))
                        .unwrap_or_else(|_| Err("native sub-window resize panicked".into()));
                        if let Err(error) = resized {
                            bridge::notify(0, "diagnostic", &error, &[]);
                            render.remove_child(key);
                            registry.borrow_mut().request_close(key);
                        }
                        registry.borrow_mut().resize(key, w, h);
                        deadline = Some(Instant::now());
                    }
                    Message::WinDetach(key) => {
                        render.detach_child(key);
                        registry.borrow_mut().detach(key);
                        deadline = Some(Instant::now());
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
                    Message::Input(mut value) => {
                        #[cfg(feature = "host-recovery-test")]
                        if value.kind == "key" && value.action == 0 && value.ctrl && value.alt {
                            if value.code == 2032 {
                                // Ctrl+Alt+P: panic inside the next UI pass.
                                fault_injection::request();
                                deadline = Some(Instant::now());
                                return Ok(());
                            }
                            if value.code == 2023 {
                                // Ctrl+Alt+G: mark the shared device lost.
                                render.inject_fault();
                                deadline = Some(Instant::now());
                                return Ok(());
                            }
                        }
                        if value.win != 0 {
                            // Pointer/touch/pinch input belongs to that sub-window's own
                            // egui viewport; keyboard shortcuts stay with the root.
                            let key = std::mem::take(&mut value.win);
                            let kind = value.kind.clone();
                            match kind.as_str() {
                                "key" => {}
                                "win_close" => {
                                    registry.borrow_mut().request_close(key);
                                    deadline = Some(Instant::now());
                                    return Ok(());
                                }
                                "blur" => {
                                    registry.borrow_mut().cancel(key);
                                    deadline = Some(Instant::now());
                                    return Ok(());
                                }
                                _ => {
                                    registry.borrow_mut().push_input(
                                        key,
                                        value,
                                        ctx.pixels_per_point(),
                                    );
                                    deadline = Some(Instant::now());
                                    return Ok(());
                                }
                            }
                        }
                        if value.kind == "drop_hover" {
                            if let Ok(feedback) =
                                drop_import::feedback(&app, &ctx, value.x, value.y, value.code)
                            {
                                bridge::notify(0, "drop_target", &feedback, &[]);
                            }
                            return Ok(());
                        }
                        if value.kind == "system_theme" {
                            appearance.publish_token(&value.text);
                            deadline = Some(Instant::now());
                            return Ok(());
                        }
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
                    Message::Drop(name, data, x, y) => {
                        drop_import::receive(&mut app, &ctx, name, data, x, y)?;
                        deadline = Some(Instant::now());
                    }
                    Message::DropFile(name, file, x, y) => {
                        drop_import::receive(&mut app, &ctx, name, file_open::read(file)?, x, y)?;
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
                        if render.recovery.halted {
                            deadline = None;
                            return Ok(());
                        }
                        if !render.attached() {
                            return Ok(());
                        }
                        let now = Instant::now();
                        let next = render.recovery.frame_deadline(now);
                        if next > now {
                            deadline = Some(next);
                            return Ok(());
                        }
                        if let Some(fault) = render.fault() {
                            return Err(fault);
                        }
                        let size = egui::vec2(
                            render.width as f32 / (config.scale * ctx.zoom_factor()),
                            render.height as f32 / (config.scale * ctx.zoom_factor()),
                        );
                        let mut raw = egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            time: Some(started.elapsed().as_secs_f64()),
                            events: std::mem::take(&mut events),
                            system_theme: appearance.current(),
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
                        registry.borrow_mut().begin_frame(
                            started.elapsed().as_secs_f64(),
                            appearance.current(),
                            ctx.zoom_factor(),
                        );
                        ui_pass_in_progress.set(true);
                        let mut output = editor_frame(&ctx, &mut app, raw);
                        ui_pass_in_progress.set(false);
                        // Child passes ran inside the root pass: collect their results, close
                        // windows the app stopped showing, and tell ArkTS.
                        let finished = registry.borrow_mut().finish_frame(&output.viewport_output);
                        for key in &finished.closed {
                            render.remove_child(*key);
                        }
                        for command in registry.borrow_mut().take_outbox() {
                            bridge::notify(0, "window", &command.json(), &[]);
                        }
                        for url in &finished.urls {
                            bridge::notify(0, "url", url, &[]);
                        }
                        // Preferences and menu actions have applied the theme by this point.
                        if let Some(color) = chrome.update(app.ui.theme) {
                            bridge::notify(0, "chrome", &color, &[]);
                        }
                        // Texture deltas apply in pass order: start-up atlas, child windows
                        // (they ran inside the root pass), then the root pass itself.
                        {
                            let mut textures = std::mem::take(&mut initial_textures);
                            textures.append(finished.textures);
                            textures.append(std::mem::take(&mut output.textures_delta));
                            output.textures_delta = textures;
                        }
                        if let Some(icon) =
                            cursor.update(input.cursor_icon(output.platform_output.cursor_icon))
                        {
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
                        // Sub-windows repaint with the root pass; honour their repaint requests.
                        let delay = output
                            .viewport_output
                            .iter()
                            .filter(|(id, v)| {
                                **id == egui::ViewportId::ROOT
                                    || v.class == egui::ViewportClass::Immediate
                            })
                            .map(|(_, v)| v.repaint_delay)
                            .min()
                            .unwrap_or(Duration::MAX);
                        let painted = render.paint(&ctx, output, finished.frames)?;
                        // A sub-window whose surface failed is asked to close; the root keeps going.
                        let failed = render.take_failed_children();
                        if !failed.is_empty() {
                            let mut registry = registry.borrow_mut();
                            for key in failed {
                                registry.request_close(key);
                            }
                            deadline = Some(Instant::now());
                        }
                        if painted {
                            if let Some(next) = Instant::now().checked_add(delay) {
                                deadline = Some(deadline.map_or(next, |d| d.min(next)));
                            }
                        }
                    }
                }
                Ok(())
            }));
        let failure = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) if rendering_operation => Some(error),
            Ok(Err(error)) => {
                app.ui.status = error.clone();
                app.ui.status_error = true;
                bridge::notify(0, "error", &error, &[]);
                None
            }
            Err(_) => Some(
                "Native rendering panic; GPU drawing stopped; restart PhotoCraft to recover".into(),
            ),
        };
        if let Some(reason) = failure {
            bridge::notify(
                0,
                "diagnostic",
                &format!("Native rendering recovery: {reason}"),
                &[],
            );
            recovery::stop_gpu(&mut app, &reason);
            if ui_pass_in_progress.get() {
                // A panic inside a sub-window pass leaves its egui pass open too.
                let nested = std::mem::take(&mut registry.borrow_mut().depth);
                for _ in 0..nested {
                    let _ = recovery::finish_interrupted_pass(&ctx);
                }
                let _ = recovery::finish_interrupted_pass(&ctx);
                ui_pass_in_progress.set(false);
                // Already mirrored at startup; replaying the older atlas over the
                // completed interrupted pass would replace newer font patches.
                initial_textures.clear();
            }
            render.halt();
            bridge::notify(
                0,
                "error",
                &format!("GPU rendering stopped: {reason}. Restart PhotoCraft to recover."),
                &[],
            );
            deadline = None;
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
        #[cfg(feature = "host-recovery-test")]
        fault_injection::check();
        app.host_ui(ui);
    })
}
#[cfg(test)]
#[path = "../tests/unit/host.rs"]
mod host_tests;
