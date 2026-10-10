//! Forward egui's final cursor to ArkUI only when it changes.
#[derive(Default)]
pub struct CursorSync {
    sent: Option<egui::CursorIcon>,
}

impl CursorSync {
    pub fn invalidate(&mut self) {
        self.sent = None;
    }

    pub fn update(&mut self, icon: egui::CursorIcon) -> Option<String> {
        if self.sent == Some(icon) {
            return None;
        }
        self.sent = Some(icon);
        Some(format!("{icon:?}"))
    }
}

#[cfg(test)]
#[path = "../tests/unit/cursor.rs"]
mod tests;
