//! egui text focus/composition <-> HarmonyOS custom-editor IME transport.
use egui::{Event, ImeEvent, Key, Modifiers};
use serde::Serialize;

#[derive(Default, Clone, PartialEq, Serialize)]
pub struct ImeState {
    active: bool,
    focus: String,
    text: String,
    start: usize,
    end: usize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    password: bool,
    #[serde(rename = "showRequest")]
    show_request: u64,
}

#[cfg(test)]
#[path = "../tests/unit/ime.rs"]
mod tests;

#[derive(Default)]
pub struct ImeSync {
    tap: Option<egui::Pos2>,
    focused: Option<egui::Id>,
    canvas: Option<(u64, String)>,
    generation: u64,
    show_request: u64,
    text: String,
    last: Option<ImeState>,
}

fn utf16_offset(text: &str, chars: usize) -> usize {
    text.chars().take(chars).map(char::len_utf16).sum()
}

fn char_offset(text: &str, units: usize) -> usize {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += c.len_utf16();
            used <= units
        })
        .count()
}

// Resizing the surface recentres the canvas, but text near its bottom can still
// be clipped. Scroll the active view just enough to retain the editing caret.
pub fn reveal_canvas_caret(
    app: &mut photocraft_ui_egui::PhotocraftApp,
    output: &egui::PlatformOutput,
) -> bool {
    let Some(ime) = output
        .ime
        .filter(|i| i.rect == i.cursor_rect && app.ui.text_edit.is_some())
    else {
        return false;
    };
    let Some(xform) = photocraft_ui_egui::canvas::ViewXform::active(app) else {
        return false;
    };
    let visible = xform.rect.shrink(6.);
    if visible.height() < ime.cursor_rect.height() || !ime.cursor_rect.is_finite() {
        return false;
    }
    let dy = if ime.cursor_rect.bottom() > visible.bottom() {
        ime.cursor_rect.bottom() - visible.bottom()
    } else if ime.cursor_rect.top() < visible.top() {
        ime.cursor_rect.top() - visible.top()
    } else {
        return false;
    };
    let Some(view) = app
        .session
        .active_index()
        .and_then(|i| app.ui.views.get_mut(i))
    else {
        return false;
    };
    if !view.zoom.is_finite() || view.zoom <= 0. {
        return false;
    }
    view.center[1] += dy / view.zoom;
    true
}

impl ImeSync {
    pub fn pointer_input(&mut self, events: &[Event]) {
        self.tap = events.iter().rev().find_map(|event| match event {
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => Some(*pos),
            _ => None,
        });
    }
    pub fn suspend(&mut self) {
        self.focused = None;
        self.canvas = None;
        self.show_request = 0;
        self.last = None;
        self.text.clear();
        crate::bridge::notify(0, "ime", "{\"active\":false}", &[]);
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        output: &egui::PlatformOutput,
        app: Option<&photocraft_ui_egui::PhotocraftApp>,
    ) -> Option<String> {
        // The canvas Type tool publishes IMEOutput without an egui TextEdit widget.
        let canvas = app
            .and_then(|a| a.ui.text_edit.as_ref())
            .filter(|_| output.ime.is_some_and(|i| i.rect == i.cursor_rect))
            .map(|edit| (edit.layer, edit.session.clone()));
        let focused = if canvas.is_some() {
            None
        } else {
            output.ime.and_then(|_| ctx.memory(|m| m.focused()))
        };
        if self.focused != focused || self.canvas != canvas {
            self.generation = self.generation.saturating_add(1);
            self.show_request = 0;
            self.text.clear();
            self.focused = focused;
            self.canvas.clone_from(&canvas);
        }
        let mut state = ImeState::default();
        if let Some(ime) = output.ime
            && (focused.is_some() || canvas.is_some())
        {
            if let Some(id) = focused {
                for event in &output.events {
                    let info = event.widget_info();
                    if info.typ == egui::WidgetType::TextEdit
                        && let Some(text) = &info.current_text_value
                    {
                        self.text.clone_from(text);
                    }
                }
                state.active = true;
                state.focus = format!("{}:{}", id.value(), self.generation);
                state.text.clone_from(&self.text);
                if let Some(range) = egui::text_edit::TextEditState::load(ctx, id)
                    .and_then(|s| s.cursor.char_range())
                {
                    let range = range.as_sorted_char_range();
                    state.start = utf16_offset(&self.text, range.start.0);
                    state.end = utf16_offset(&self.text, range.end.0);
                }
            } else if let Some(app) = app
                && let Some(edit) = &app.ui.text_edit
            {
                use photocraft_engine::doc::{LayerContent, LayerId};
                if let Some(layer) = app
                    .session
                    .active()
                    .and_then(|s| s.doc.layer(LayerId(edit.layer)))
                    && let LayerContent::Text(text) = &layer.content
                {
                    self.text.clone_from(&text.text);
                }
                state.active = true;
                state.focus = format!("type:{}:{}:{}", edit.layer, edit.session, self.generation);
                state.text.clone_from(&self.text);
                state.start = utf16_offset(&self.text, edit.anchor.min(edit.caret));
                state.end = utf16_offset(&self.text, edit.anchor.max(edit.caret));
            }
            // Help opens directly into its search field; request the keyboard once for
            // each focus session. Other widget editors open it on an explicit tap.
            // Read the original host events: egui may discard click state in a layout
            // retry before the final output is delivered.
            let editor_rect = focused
                .and_then(|id| ctx.read_response(id).map(|r| r.rect))
                .unwrap_or(ime.rect);
            let tapped = self.tap.take().is_some_and(|p| editor_rect.contains(p));
            let help_search = focused == Some(egui::Id::new("help-menu-search").with("field"));
            if tapped || ((canvas.is_some() || help_search) && self.show_request == 0) {
                self.show_request = self.show_request.saturating_add(1);
            }
            state.show_request = self.show_request;
            let scale = ctx.pixels_per_point();
            state.x = ime.cursor_rect.left() * scale;
            state.y = ime.cursor_rect.top() * scale;
            state.width = ime.cursor_rect.width().max(1.) * scale;
            state.height = ime.cursor_rect.height().max(1.) * scale;
            state.password = ime.purpose == egui::IMEPurpose::Password;
        }
        if self.last.as_ref() == Some(&state) {
            return None;
        }
        let json = serde_json::to_string(&state).ok()?;
        self.last = Some(state);
        Some(json)
    }

    pub fn input(
        &mut self,
        ctx: &egui::Context,
        mut app: Option<&mut photocraft_ui_egui::PhotocraftApp>,
        input: crate::input::Input,
        events: &mut Vec<Event>,
    ) {
        let Some(state) = self.last.as_ref() else {
            return;
        };
        if !state.active
            || state.focus != input.focus
            || (self.canvas.is_none() && ctx.memory(|m| m.focused()) != self.focused)
        {
            return;
        }
        if let Some((layer, session)) = &self.canvas
            && app
                .as_ref()
                .and_then(|a| a.ui.text_edit.as_ref())
                .is_none_or(|e| e.layer != *layer || e.session != *session)
        {
            return;
        }
        match input.kind.as_str() {
            "ime_commit" => events.push(Event::Ime(ImeEvent::Commit(input.text))),
            "ime_preedit" => events.push(Event::Ime(ImeEvent::Preedit {
                text: input.text,
                active_range_chars: None,
            })),
            "ime_delete_left" | "ime_delete_right" => {
                let before = input.kind == "ime_delete_left";
                let at = if before { state.start } else { state.end };
                let count = input.count.min(state.text.encode_utf16().count());
                let chars = if before {
                    char_offset(&state.text, at)
                        .saturating_sub(char_offset(&state.text, at.saturating_sub(count)))
                } else {
                    char_offset(&state.text, at.saturating_add(count))
                        .saturating_sub(char_offset(&state.text, at))
                };
                if self.canvas.is_some() {
                    let count = if state.start != state.end { 1 } else { chars };
                    for _ in 0..count.min(65536) {
                        key_events(events, if before { Key::Backspace } else { Key::Delete });
                    }
                } else {
                    events.push(Event::Ime(ImeEvent::DeleteSurrounding {
                        before_chars: if before { chars } else { 0 },
                        after_chars: if before { 0 } else { chars },
                    }));
                }
            }
            "ime_select" => {
                if let Some(id) = self.focused
                    && let Some(mut edit) = egui::text_edit::TextEditState::load(ctx, id)
                {
                    edit.cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(char_offset(&state.text, input.start)),
                            egui::text::CCursor::new(char_offset(&state.text, input.end)),
                        )));
                    edit.store(ctx, id);
                }
                if self.canvas.is_some()
                    && let Some(edit) = app.as_mut().and_then(|a| a.ui.text_edit.as_mut())
                {
                    edit.anchor = char_offset(&state.text, input.start);
                    edit.caret = char_offset(&state.text, input.end);
                }
            }
            "ime_key" => {
                let key = match input.code {
                    1 => Key::ArrowUp,
                    2 => Key::ArrowDown,
                    3 => Key::ArrowLeft,
                    4 => Key::ArrowRight,
                    _ => Key::Enter,
                };
                key_events(events, key);
            }
            _ => {}
        }
    }
}

fn key_events(events: &mut Vec<Event>, key: Key) {
    for pressed in [true, false] {
        events.push(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
}
