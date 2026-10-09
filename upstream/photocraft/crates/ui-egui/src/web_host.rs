//! Host hooks for the web shell. File › Exit sends `ViewportCommand::Close`, which browsers and
//! ArkWeb ignore (`window.close` is blocked). When a host object is present (the HarmonyOS
//! wrapper registers `photocraftHost.quit`), the close is forwarded there.
//!
//! `photocraftHost.setChrome` paints the status bar and the navigation-indicator avoid area in
//! the theme's window chrome. A plain browser has no such method.
//!
//! A Save answer on the unsaved-changes prompt downloads the file, and the wrapper then shows the
//! system save picker. [`on_close_requested`] must not call `quit` until that picker has finished:
//! `terminateSelf` would destroy it, so the file is never written.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wasm_bindgen::JsCast as _;

use crate::PhotocraftApp;
use crate::theme::ThemeKind;

struct ChromeSync {
    sent: Option<ThemeKind>,
    misses: u8,
}

static CHROME_SYNC: Mutex<ChromeSync> = Mutex::new(ChromeSync { sent: None, misses: 0 });
const CHROME_GIVE_UP: u8 = 8;

static QUIT_SENT: AtomicBool = AtomicBool::new(false);
/// The save picker was cancelled after quit had already been requested. Drop the quit so the
/// next File › Exit can ask again.
static ABORT_QUIT: AtomicBool = AtomicBool::new(false);

/// Tell the HarmonyOS wrapper the theme chrome colour. Called every frame; the host is asked
/// only when the colour changes. A browser has no `setChrome`, so a few misses stop the lookup.
pub fn sync_chrome(kind: ThemeKind) {
    let mut sync = CHROME_SYNC.lock().unwrap_or_else(|e| e.into_inner());
    if sync.sent == Some(kind) || (sync.sent.is_none() && sync.misses >= CHROME_GIVE_UP) {
        return;
    }
    if call_host_chrome(&kind.chrome_css()) {
        sync.sent = Some(kind);
        sync.misses = 0;
    } else {
        sync.misses = sync.misses.saturating_add(1);
    }
}

fn call_host_chrome(color: &str) -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Ok(host) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("photocraftHost")) else {
        return false;
    };
    if host.is_undefined() || host.is_null() {
        return false;
    }
    let Ok(func) = js_sys::Reflect::get(&host, &wasm_bindgen::JsValue::from_str("setChrome")) else {
        return false;
    };
    let Some(func) = func.dyn_ref::<js_sys::Function>() else { return false };
    func.call1(&host, &wasm_bindgen::JsValue::from_str(color)).is_ok()
}

/// The wrapper reported that the save the user was quitting through was cancelled.
pub fn abort_quit() {
    QUIT_SENT.store(false, Ordering::Relaxed);
    ABORT_QUIT.store(true, Ordering::Relaxed);
}

/// After [`crate::discard_ui::guard_window_close`]: quit through the host once the user has
/// confirmed, or immediately when there is no unsaved work. A host save picker still open holds
/// the quit (see `save_dialog`).
pub fn on_close_requested(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if ABORT_QUIT.swap(false, Ordering::Relaxed) {
        app.allow_close = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    }
    if app.discard.as_ref().is_some_and(|p| p.awaiting_save.is_some()) {
        ctx.request_repaint_after(Duration::from_millis(32));
        return;
    }
    if app.allow_close || (ctx.input(|i| i.viewport().close_requested()) && app.discard.is_none()) {
        quit();
    }
}

fn quit() {
    if QUIT_SENT.swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    if call_host_quit(&window) {
        return;
    }
    let _ = window.close();
}

fn call_host_quit(window: &web_sys::Window) -> bool {
    let Ok(host) = js_sys::Reflect::get(window, &wasm_bindgen::JsValue::from_str("photocraftHost")) else {
        return false;
    };
    if host.is_undefined() || host.is_null() {
        return false;
    }
    let Ok(func) = js_sys::Reflect::get(&host, &wasm_bindgen::JsValue::from_str("quit")) else {
        return false;
    };
    let Some(func) = func.dyn_ref::<js_sys::Function>() else {
        return false;
    };
    func.call0(&host).is_ok()
}
