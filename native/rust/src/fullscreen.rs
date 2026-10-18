//! Optional window full screen: the egui status-bar button and Window menu item ask ArkTS to
//! enter / leave HarmonyOS full screen, and ArkTS reports the real state back (including when the
//! system leaves full screen by itself), so icon and menu text always match the window.
//!
//! Default is off. The state is only a mirror of the host window; it is never changed optimistically.

use photocraft_ui_egui::window_fullscreen::WindowFullscreen;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Rust → ArkTS notification kind; the text is [`ENTER`] or [`EXIT`].
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub const NOTIFY_KIND: &str = "fullscreen";
/// ArkTS → Rust input kind; the text is `on` or `off` (see [`parse_state`]).
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub const INPUT_KIND: &str = "fullscreen_state";
pub const ENTER: &str = "enter";
pub const EXIT: &str = "exit";

/// `on` / `true` / `1` and `off` / `false` / `0`; anything else is ignored.
pub fn parse_state(token: &str) -> Option<bool> {
    match token.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "1" => Some(true),
        "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

#[derive(Clone, Default)]
pub struct Fullscreen {
    active: Arc<AtomicBool>,
}

impl Fullscreen {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Store the state ArkTS reported; true when it changed (the caller repaints).
    pub fn report(&self, token: &str) -> bool {
        match parse_state(token) {
            Some(on) => self.active.swap(on, Ordering::Relaxed) != on,
            None => false,
        }
    }

    /// The service for the editor. `ask` receives [`ENTER`] or [`EXIT`].
    pub fn service(&self, ask: impl Fn(&str) + 'static) -> WindowFullscreen {
        let state = Arc::clone(&self.active);
        WindowFullscreen {
            is_active: Box::new(move || state.load(Ordering::Relaxed)),
            request: Box::new(move |on| ask(if on { ENTER } else { EXIT })),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/fullscreen.rs"]
mod tests;
