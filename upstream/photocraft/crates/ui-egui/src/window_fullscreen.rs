//! Optional host-window full screen (HarmonyOS system bars hidden). The host injects
//! [`WindowFullscreen`] through `Services`; without it nothing here is shown, so every other
//! platform is unchanged. The host owns the real window state: [`request`] only asks, and the host
//! reports the outcome (also when the system leaves full screen by itself) through `is_active`.

use crate::{PhotocraftApp, i18n};

/// Menu command id (Window › below Theme) that toggles host full screen.
pub const COMMAND: &str = "window.fullscreen.toggle";

pub struct WindowFullscreen {
    /// Whether the host window is currently full screen (read every frame; the host requests a
    /// repaint itself when the state changes).
    pub is_active: Box<dyn Fn() -> bool>,
    /// Ask the host to enter (`true`) or leave (`false`) full screen.
    pub request: Box<dyn Fn(bool)>,
}

/// Whether the host offers full screen at all.
pub fn available(app: &PhotocraftApp) -> bool {
    app.services.window_fullscreen.is_some()
}

/// The host window's current state; `false` without a host service.
pub fn active(app: &PhotocraftApp) -> bool {
    app.services.window_fullscreen.as_ref().is_some_and(|f| (f.is_active)())
}

/// English label of the action the next click performs (translated when drawn).
pub fn label(active: bool) -> &'static str {
    if active { "Exit Full Screen" } else { "Enter Full Screen" }
}

/// Icon name of the action the next click performs.
pub fn icon(active: bool) -> &'static str {
    if active { "fullscreen-exit" } else { "fullscreen-enter" }
}

/// Toggle: ask the host for the opposite of its current state.
pub fn toggle(app: &PhotocraftApp) -> bool {
    let Some(f) = app.services.window_fullscreen.as_ref() else { return false };
    let target = !(f.is_active)();
    (f.request)(target);
    target
}

/// The translated label, for tooltips and menu text.
pub fn tr_label(active: bool) -> String {
    i18n::tr(i18n::current(), label(active)).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn labels_and_icons_follow_the_next_action() {
        assert_eq!(label(false), "Enter Full Screen");
        assert_eq!(label(true), "Exit Full Screen");
        assert_eq!(icon(false), "fullscreen-enter");
        assert_eq!(icon(true), "fullscreen-exit");
        assert!(crate::icons::exists(icon(false)) && crate::icons::exists(icon(true)));
        // Distinct from the "Only show canvas" button's glyph.
        assert_ne!(icon(false), "maximize-2");
        assert_ne!(icon(true), "maximize-2");
        assert_ne!(icon(false), "canvas-only");
        assert_ne!(icon(true), "canvas-only");
    }

    #[test]
    fn toggle_requests_the_opposite_of_the_host_state() {
        let state = Rc::new(Cell::new(false));
        let asked = Rc::new(Cell::new(None));
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        assert!(!available(&app) && !toggle(&app));
        let (s, a) = (state.clone(), asked.clone());
        app.services.window_fullscreen = Some(WindowFullscreen { is_active: Box::new(move || s.get()), request: Box::new(move |on| a.set(Some(on))) });
        assert!(available(&app) && !active(&app));
        assert!(toggle(&app));
        assert_eq!(asked.get(), Some(true));
        state.set(true);
        assert!(active(&app) && !toggle(&app));
        assert_eq!(asked.get(), Some(false));
    }
}
