//! HarmonyOS colour scheme (Configuration.colorMode) for PhotoCraft's Auto appearance mode.
//!
//! ArkTS publishes `dark` / `light` at init and on `onConfigurationUpdate`. The native worker
//! stores the latest value for [`photocraft_ui_egui::Services::system_theme`] and egui RawInput.

use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU8, Ordering},
};

const UNKNOWN: u8 = 0;
const DARK: u8 = 1;
const LIGHT: u8 = 2;

#[derive(Clone)]
pub struct Appearance {
    value: Arc<AtomicU8>,
    wake: Arc<OnceLock<egui::Context>>,
}

impl Appearance {
    pub fn new(initial: Option<egui::Theme>) -> Self {
        let appearance = Self {
            value: Arc::new(AtomicU8::new(code_of(initial))),
            wake: Arc::new(OnceLock::new()),
        };
        appearance
    }

    pub fn from_token(token: &str) -> Self {
        Self::new(parse_token(token))
    }

    pub fn publish(&self, theme: Option<egui::Theme>) {
        let code = code_of(theme);
        if self.value.swap(code, Ordering::Relaxed) != code
            && let Some(ctx) = self.wake.get()
        {
            ctx.request_repaint();
        }
    }

    pub fn publish_token(&self, token: &str) {
        self.publish(parse_token(token));
    }

    pub fn current(&self) -> Option<egui::Theme> {
        decode(self.value.load(Ordering::Relaxed))
    }

    pub fn service(&self) -> photocraft_ui_egui::SystemThemeFn {
        let value = Arc::clone(&self.value);
        let wake = Arc::clone(&self.wake);
        Box::new(move |ctx| {
            let _ = wake.set(ctx.clone());
            decode(value.load(Ordering::Relaxed))
        })
    }
}

pub fn parse_token(token: &str) -> Option<egui::Theme> {
    match token.trim().to_ascii_lowercase().as_str() {
        "dark" => Some(egui::Theme::Dark),
        "light" => Some(egui::Theme::Light),
        _ => None,
    }
}

fn code_of(theme: Option<egui::Theme>) -> u8 {
    match theme {
        Some(egui::Theme::Dark) => DARK,
        Some(egui::Theme::Light) => LIGHT,
        None => UNKNOWN,
    }
}

fn decode(code: u8) -> Option<egui::Theme> {
    match code {
        DARK => Some(egui::Theme::Dark),
        LIGHT => Some(egui::Theme::Light),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/appearance.rs"]
mod tests;
