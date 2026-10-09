//! The browser shell: web `Services`, drag-and-drop, and the eframe web runner.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use photocraft_codecs::{ChannelLayout, EncodeOptions, Image};
use photocraft_doc::Document;
use photocraft_engine::Session;
use photocraft_ui_egui::save_dialog::HostSaveGate;
use photocraft_ui_egui::theme::ThemeKind;
use photocraft_ui_egui::{PhotocraftApp, Services};
use wasm_bindgen::JsCast as _;

type Inbox = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
type HostFonts = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
type CloseRequest = Arc<AtomicBool>;

/// Everything File › Open reads: PhotoCraft and Photoshop documents, flat images, and Photoshop
/// brushes (.abr) and gradients (.grd), which go to the preset libraries.
const OPEN_EXTS: &[&str] = &[
    "pcraft", "psd", "psb", "psdt", "png", "jpg", "jpeg", "tif", "tiff", "webp", "gif", "bmp", "tga", "ico", "qoi", "exr", "hdr", "pbm", "pgm", "ppm", "pam",
    "pfm", "heic", "heif", "hif", "dng", "cr2", "cr3", "nef", "nrw", "arw", "pef", "orf", "rw2", "raf", "abr", "grd",
];
const CANVAS_ID: &str = "photocraft_canvas";

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let q = query();
        let force_cpu = q.contains("cpu");
        let mut options = eframe::WebOptions::default();
        photocraft_ui_egui::gpu_canvas::use_adapter_limits(&mut options.wgpu_options.wgpu_setup);
        if q.contains("webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        let pen_target = canvas.clone();
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    PhotocraftApp::setup_context(&cc.egui_ctx, ThemeKind::Pro);
                    let inbox: Inbox = Arc::default();
                    let host_fonts: HostFonts = Arc::default();
                    let close_request: CloseRequest = Arc::default();
                    let mut app = PhotocraftApp::new(Session::new(), services(inbox.clone(), cc.egui_ctx.clone()));
                    listen_host_close(close_request.clone(), cc.egui_ctx.clone());
                    crate::stylus_input::install(&pen_target, app.stylus.feed.clone(), cc.egui_ctx.clone());
                    listen_open_files(inbox.clone(), cc.egui_ctx.clone());
                    fetch_host_fonts(host_fonts.clone(), cc.egui_ctx.clone());
                    app.set_theme(&cc.egui_ctx, ThemeKind::Pro);
                    if let Some(rs) = cc.wgpu_render_state.clone()
                        && !force_cpu
                    {
                        log::info!("photocraft-web: wgpu backend {:?}", rs.adapter.get_info().backend);
                        app.set_wgpu(rs);
                    }
                    Ok(Box::new(WebShell { app, inbox, host_fonts, close_request }))
                }),
            )
            .await;
        if let Some(el) = document.get_element_by_id("photocraft_loading") {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>PhotoCraft failed to start: {e:?}</p><p>A browser with WebGPU or WebGL2 is required.</p>")),
            }
        }
    });
}

/// Files opened with PhotoCraft from the system file manager. The wrapper posts a message port,
/// then a name (`open:…`) and the file bytes.
fn listen_open_files(inbox: Inbox, ctx: egui::Context) {
    use std::sync::{Arc, Mutex};
    use wasm_bindgen::closure::Closure;
    if !host_present() {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let pending_name = Arc::new(Mutex::new(None));
    let cb = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
        let data = e.data();
        if data.as_string().as_deref() != Some("photocraft-open-port") {
            return;
        }
        let ports = e.ports();
        if ports.length() == 0 {
            return;
        }
        let Ok(port) = ports.get(0).dyn_into::<web_sys::MessagePort>() else { return };
        let inbox = inbox.clone();
        let ctx = ctx.clone();
        let pending_name = pending_name.clone();
        let on_file = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
            let data = ev.data();
            if let Some(text) = data.as_string() {
                if let Some(name) = text.strip_prefix("open:") {
                    *pending_name.lock().unwrap_or_else(|e| e.into_inner()) = Some(open_file_name(name));
                }
                return;
            }
            let Some(bytes) = js_bytes(&data) else { return };
            let name = pending_name.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_else(|| "opened".into());
            inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
            ctx.request_repaint();
        });
        port.set_onmessage(Some(on_file.as_ref().unchecked_ref()));
        port.start();
        let _ = port.post_message(&wasm_bindgen::JsValue::from_str("ready"));
        on_file.forget();
    });
    if window.add_event_listener_with_callback("message", cb.as_ref().unchecked_ref()).is_ok() {
        cb.forget();
        request_open_files();
    }
}

fn open_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    if base.is_empty() || base == "." || base == ".." { "opened".into() } else { base.to_string() }
}

fn js_bytes(value: &wasm_bindgen::JsValue) -> Option<Vec<u8>> {
    if let Some(buffer) = value.dyn_ref::<js_sys::ArrayBuffer>() {
        return Some(js_sys::Uint8Array::new(buffer).to_vec());
    }
    value.dyn_ref::<js_sys::Uint8Array>().map(|array| array.to_vec())
}

fn request_open_files() {
    let Some(window) = web_sys::window() else { return };
    let Ok(host) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("photocraftHost")) else { return };
    if host.is_undefined() || host.is_null() {
        return;
    }
    let Ok(func) = js_sys::Reflect::get(&host, &wasm_bindgen::JsValue::from_str("pullOpens")) else { return };
    let Some(func) = func.dyn_ref::<js_sys::Function>() else { return };
    let _ = func.call0(&host);
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

/// Same-origin faces served by the HarmonyOS wrapper (`/system/fonts/HarmonyOS_Sans_{SC,TC}.ttf`).
fn fetch_host_fonts(pending: HostFonts, ctx: egui::Context) {
    for (script, path) in [("zh-hans", "fonts/HarmonyOS_Sans_SC.ttf"), ("zh-hant", "fonts/HarmonyOS_Sans_TC.ttf")] {
        let pending = pending.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(bytes) = fetch_font(path).await else { return };
            log::info!("host CJK font {path}: {} bytes", bytes.len());
            pending.lock().unwrap_or_else(|e| e.into_inner()).push((script.to_owned(), bytes));
            ctx.request_repaint();
        });
    }
}

async fn fetch_font(path: &str) -> Option<Vec<u8>> {
    let window = web_sys::window()?;
    let href = window.location().href().ok()?;
    let url = web_sys::Url::new_with_base(path, &href).ok()?;
    let value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&url.href())).await.ok()?;
    let response: web_sys::Response = value.dyn_into().ok()?;
    if !response.ok() {
        return None;
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().ok()?).await.ok()?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    photocraft_ui_egui::cjk_fonts::font_bytes_ok(&bytes).then_some(bytes)
}

/// HarmonyOS Sans arrives asynchronously. Register it after the frame's language is applied.
fn install_host_fonts(pending: &HostFonts, ctx: &egui::Context) {
    let ready = std::mem::take(&mut *pending.lock().unwrap_or_else(|e| e.into_inner()));
    if ready.is_empty() {
        return;
    }
    let mut stored = false;
    for (script, bytes) in ready {
        if photocraft_ui_egui::cjk_fonts::store_host_cjk_font(&script, bytes) {
            stored = true;
        }
    }
    if stored {
        photocraft_ui_egui::cjk_fonts::reapply_host_fonts(ctx);
    }
}

/// Wraps the app to read dropped files asynchronously (browsers can't read them synchronously,
/// so the app's own drop path can't handle them) and feed them through the inbox.
struct WebShell {
    app: PhotocraftApp,
    inbox: Inbox,
    host_fonts: HostFonts,
    close_request: CloseRequest,
}

impl eframe::App for WebShell {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        for f in dropped {
            let inbox = self.inbox.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
                match f.bytes_async().await {
                    Ok(bytes) => {
                        inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                        ctx.request_repaint();
                    }
                    Err(e) => log::error!("couldn't read dropped file {name}: {e}"),
                }
            });
        }
        self.app.logic(ctx, frame);
        if self.close_request.swap(false, Ordering::Relaxed) {
            if let Err(error) = photocraft_ui_egui::menus::invoke(&mut self.app, ctx, "file.exit", serde_json::Value::Null) {
                log::error!("system close request failed: {error}");
            }
            ctx.request_repaint();
        }
        // After prefs have applied the saved theme, so the first paint is not the default Pro chrome.
        photocraft_ui_egui::web_host::sync_chrome(self.app.ui.theme);
        install_host_fonts(&self.host_fonts, ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
}

/// A HarmonyOS window close request uses the same guarded File › Exit command as the menu.
fn listen_host_close(close_request: CloseRequest, ctx: egui::Context) {
    use wasm_bindgen::closure::Closure;
    let Some(window) = web_sys::window() else { return };
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        close_request.store(true, Ordering::Relaxed);
        ctx.request_repaint();
    });
    if window.add_event_listener_with_callback("photocraft-request-close", cb.as_ref().unchecked_ref()).is_ok() {
        cb.forget();
    }
}

fn services(inbox: Inbox, ctx: egui::Context) -> Services {
    let open_inbox = inbox.clone();
    // HarmonyOS shows the system save picker after the download. Quit must wait for it.
    let gate = HostSaveGate::new();
    let write_gate = std::sync::Arc::clone(&gate);
    let host = host_present();
    if host {
        listen_export(std::sync::Arc::clone(&gate), ctx.clone());
    }
    let save_dialog = host.then(|| gate.dialog());
    Services {
        import: Some(Box::new(|name: &str, bytes: &[u8]| photocraft_io::import(name, bytes).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string()))),
        export: Some(Box::new(|doc: &Document, path: &str, settings: &photocraft_ui_egui::ExportSettings| {
            let mut opts = photocraft_io::ExportOptions::default();
            if let Some(q) = settings.jpeg_quality {
                opts.encode.jpeg_quality = q;
            }
            opts.encode.webp_lossless = settings.webp_lossless;
            if let Some(q) = settings.webp_quality {
                opts.encode.webp_quality = q;
            }
            opts.tiff_layers = settings.tiff_layers;
            opts.xmp = if settings.xmp_all { photocraft_io::XmpEmbed::All } else { photocraft_io::XmpEmbed::None };
            photocraft_io::export(doc, path, &opts).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string())
        })),
        pick_open: Some(Box::new(move || {
            let inbox = open_inbox.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let Some(file) = rfd::AsyncFileDialog::new().add_filter("All Formats", OPEN_EXTS).pick_file().await else {
                    return;
                };
                let bytes = file.read().await;
                inbox.lock().unwrap_or_else(|e| e.into_inner()).push((file.file_name(), bytes));
                ctx.request_repaint();
            });
            None
        })),
        // No save dialog in the page: the suggested name becomes the download name.
        // HarmonyOS shows the system save picker afterwards; File › Quit waits for it (`save_dialog`).
        pick_save: Some(Box::new(|suggested: &str| {
            let name = std::path::Path::new(suggested).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| suggested.to_string());
            Some(name)
        })),
        write: Some(Box::new(move |path: &str, bytes: &[u8]| download(path, bytes, &write_gate))),
        save_dialog,
        encode_png: Some(Box::new(|w, h, rgba| {
            let img = Image::from_u8(w, h, ChannelLayout::Rgba, rgba.to_vec()).map_err(|e| e.to_string())?;
            photocraft_codecs::encode(&img, photocraft_codecs::Format::Png, &EncodeOptions::default()).map_err(|e| e.to_string())
        })),
        inbox: Some(inbox),
        // Preferences live in the browser's localStorage.
        load_prefs: Some(Box::new(|| local_storage()?.get_item(PREFS_KEY).ok().flatten())),
        save_prefs: Some(Box::new(|text: &str| local_storage().ok_or("no localStorage")?.set_item(PREFS_KEY, text).map_err(|e| format!("{e:?}")))),
        ..Default::default()
    }
}

const PREFS_KEY: &str = "photocraft.preferences";

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// The HarmonyOS wrapper injects `photocraftHost`. A plain browser has no save picker after download.
fn host_present() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Ok(host) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("photocraftHost")) else {
        return false;
    };
    !(host.is_undefined() || host.is_null())
}

/// Tell the wrapper which download the unsaved-changes prompt is waiting on.
fn expect_host_save(name: &str) {
    let Some(window) = web_sys::window() else { return };
    let Ok(host) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("photocraftHost")) else { return };
    if host.is_undefined() || host.is_null() {
        return;
    }
    let Ok(func) = js_sys::Reflect::get(&host, &wasm_bindgen::JsValue::from_str("expectSave")) else { return };
    let Some(func) = func.dyn_ref::<js_sys::Function>() else { return };
    let _ = func.call1(&host, &wasm_bindgen::JsValue::from_str(name));
}

/// `photocraft-export` from the wrapper: `{result, name}` or the older `"saved"` / `"cancelled"`.
fn export_detail(detail: &wasm_bindgen::JsValue) -> (Option<bool>, String) {
    if let Some(text) = detail.as_string() {
        let saved = match text.as_str() {
            "saved" => Some(true),
            "cancelled" => Some(false),
            _ => None,
        };
        return (saved, String::new());
    }
    let result = js_sys::Reflect::get(detail, &wasm_bindgen::JsValue::from_str("result")).ok().and_then(|v| v.as_string());
    let name = js_sys::Reflect::get(detail, &wasm_bindgen::JsValue::from_str("name")).ok().and_then(|v| v.as_string()).unwrap_or_default();
    let saved = match result.as_deref() {
        Some("saved") => Some(true),
        Some("cancelled") => Some(false),
        _ => None,
    };
    (saved, name)
}

fn listen_export(gate: std::sync::Arc<HostSaveGate>, ctx: egui::Context) {
    use wasm_bindgen::closure::Closure;
    let Some(window) = web_sys::window() else { return };
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        let Ok(detail) = js_sys::Reflect::get(&e, &wasm_bindgen::JsValue::from_str("detail")) else { return };
        let (saved, name) = export_detail(&detail);
        let Some(saved) = saved else { return };
        if gate.finish(&name, saved) && !saved {
            photocraft_ui_egui::web_host::abort_quit();
        }
        ctx.request_repaint();
    });
    if window.add_event_listener_with_callback("photocraft-export", cb.as_ref().unchecked_ref()).is_ok() {
        cb.forget();
    }
}

/// Trigger a browser download of `bytes` named after the last component of `path`.
fn download(path: &str, bytes: &[u8], gate: &HostSaveGate) -> Result<(), String> {
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "photocraft".into());
    if gate.note_if_armed(&name) {
        expect_host_save(&name);
    }
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime_for(&name));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(&name);
    a.style().set_property("display", "none").map_err(js)?;
    let body = document.body().ok_or("no body")?;
    body.append_child(&a).map_err(js)?;
    a.click();
    a.remove();
    // Revoke after the click has been dispatched; the download keeps its own reference.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        web_sys::Url::revoke_object_url(&url).ok();
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000).map_err(js)?;
    Ok(())
}

fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("tif" | "tiff") => "image/tiff",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("psd" | "psb") => "image/vnd.adobe.photoshop",
        _ => "application/octet-stream",
    }
}
