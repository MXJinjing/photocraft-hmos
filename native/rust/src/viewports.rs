//! Native sub-window bookkeeping for egui immediate viewports (floating document windows).
//!
//! egui calls the renderer installed by [`install`] from inside the root pass whenever the
//! app shows an immediate viewport with `embed_viewports == false`. Each viewport maps to
//! one HarmonyOS sub-window (own XComponent / NativeWindow / wgpu surface). This module only
//! tracks state, input and egui passes; ArkTS creates the windows ([`Command`]) and
//! `render.rs` owns the surfaces.
// The worker that drives this module only exists on the OHOS target.
#![cfg_attr(not(target_env = "ohos"), allow(dead_code))]
use crate::input;
use egui::{
    Event, FullOutput, Pos2, RawInput, Rect, Vec2, ViewportBuilder, ViewportEvent, ViewportId,
    ViewportInfo, epaint::ClippedShape, vec2,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Window size (points) used until the sub-window reports its real surface.
const DEFAULT_SIZE: Vec2 = vec2(800.0, 600.0);

/// Requests for the ArkTS `SubWindowHost` (sent as `window` notifications).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Open {
        key: u32,
        title: String,
        width: u32,
        height: u32,
        x: Option<i32>,
        y: Option<i32>,
    },
    Title {
        key: u32,
        title: String,
    },
    Resize {
        key: u32,
        width: u32,
        height: u32,
    },
    Close {
        key: u32,
    },
}
impl Command {
    pub fn json(&self) -> String {
        match self {
            Command::Open {
                key,
                title,
                width,
                height,
                x,
                y,
            } => serde_json::json!({
                "op": "open", "win": key, "title": title,
                "width": width, "height": height, "x": x, "y": y,
            }),
            Command::Title { key, title } => {
                serde_json::json!({"op": "title", "win": key, "title": title})
            }
            Command::Resize { key, width, height } => {
                serde_json::json!({"op": "resize", "win": key, "width": width, "height": height})
            }
            Command::Close { key } => serde_json::json!({"op": "close", "win": key}),
        }
        .to_string()
    }
}

/// One child pass, ready to draw into the sub-window's surface.
pub struct ChildFrame {
    pub key: u32,
    pub shapes: Vec<ClippedShape>,
    pub pixels_per_point: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `open` was sent; the XComponent surface does not exist yet.
    Opening,
    Attached,
    /// Surface destroyed (e.g. app backgrounded); the window stays registered.
    Detached,
}

struct Child {
    key: u32,
    title: String,
    phase: Phase,
    size_px: (u32, u32),
    input: input::State,
    events: Vec<Event>,
    close_requested: bool,
    used: bool,
}

pub struct Registry {
    children: HashMap<ViewportId, Child>,
    by_key: HashMap<u32, ViewportId>,
    next_key: u32,
    outbox: Vec<Command>,
    scale: f32,
    zoom: f32,
    time: f64,
    theme: Option<egui::Theme>,
    textures: egui::TexturesDelta,
    frames: HashMap<u32, ChildFrame>,
    urls: Vec<String>,
    feed: photocraft_ui_egui::stylus::StylusFeed,
    /// Number of nested child passes currently running (non-zero after a panic).
    pub depth: usize,
}

impl Registry {
    pub fn new(scale: f32, feed: photocraft_ui_egui::stylus::StylusFeed) -> Self {
        Self {
            children: HashMap::new(),
            by_key: HashMap::new(),
            next_key: 1,
            outbox: Vec::new(),
            scale,
            zoom: 1.0,
            time: 0.0,
            theme: None,
            textures: Default::default(),
            frames: HashMap::new(),
            urls: Vec::new(),
            feed,
            depth: 0,
        }
    }

    fn ppp(&self) -> f32 {
        self.scale * self.zoom
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Start of a root frame: clear per-frame bookkeeping.
    pub fn begin_frame(&mut self, time: f64, theme: Option<egui::Theme>, zoom: f32) {
        self.time = time;
        self.theme = theme;
        self.zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
        self.frames.clear();
        for child in self.children.values_mut() {
            child.used = false;
        }
    }

    /// Build the raw input for a child pass. Called from the immediate-viewport renderer.
    fn prepare(&mut self, id: ViewportId, parent: ViewportId, builder: &ViewportBuilder) -> RawInput {
        let ppp = self.ppp();
        let title = builder.title.clone().unwrap_or_default();
        let wanted = builder.inner_size.unwrap_or(DEFAULT_SIZE);
        if !self.children.contains_key(&id) {
            let key = self.next_key;
            self.next_key = self.next_key.wrapping_add(1).max(1);
            let px = |points: f32| (points * ppp).round().clamp(1.0, 16384.0) as u32;
            self.outbox.push(Command::Open {
                key,
                title: title.clone(),
                width: px(wanted.x),
                height: px(wanted.y),
                x: builder.position.map(|p| (p.x * ppp).round() as i32),
                y: builder.position.map(|p| (p.y * ppp).round() as i32),
            });
            self.by_key.insert(key, id);
            self.children.insert(
                id,
                Child {
                    key,
                    title: title.clone(),
                    phase: Phase::Opening,
                    size_px: (px(wanted.x), px(wanted.y)),
                    input: {
                        let mut state = input::State::default();
                        state.feed = self.feed.clone();
                        state
                    },
                    events: Vec::new(),
                    close_requested: false,
                    used: true,
                },
            );
        }
        let Some(child) = self.children.get_mut(&id) else {
            return RawInput::default();
        };
        child.used = true;
        if child.title != title {
            child.title = title;
            self.outbox.push(Command::Title {
                key: child.key,
                title: child.title.clone(),
            });
        }
        let size = vec2(
            child.size_px.0 as f32 / ppp,
            child.size_px.1 as f32 / ppp,
        );
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        let mut info = ViewportInfo {
            parent: Some(parent),
            title: Some(child.title.clone()),
            native_pixels_per_point: Some(self.scale),
            inner_rect: Some(rect),
            focused: Some(true),
            ..Default::default()
        };
        if std::mem::take(&mut child.close_requested) {
            info.events.push(ViewportEvent::Close);
        }
        let mut raw = RawInput {
            viewport_id: id,
            screen_rect: Some(rect),
            time: Some(self.time),
            events: std::mem::take(&mut child.events),
            system_theme: self.theme,
            ..Default::default()
        };
        raw.viewports.insert(id, info);
        raw
    }

    /// Stash a finished child pass: textures in pass order, shapes of the latest pass.
    fn collect(&mut self, id: ViewportId, mut output: FullOutput) {
        self.textures
            .append(std::mem::take(&mut output.textures_delta));
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::OpenUrl(url) = command {
                self.urls.push(url.url.clone());
            }
        }
        if let Some(child) = self.children.get(&id) {
            self.frames.insert(
                child.key,
                ChildFrame {
                    key: child.key,
                    shapes: output.shapes,
                    pixels_per_point: output.pixels_per_point,
                },
            );
        }
    }

    /// End of the root frame. Closes windows the app stopped showing and applies title/size
    /// commands the app queued for them.
    pub fn finish_frame(
        &mut self,
        viewports: &egui::OrderedViewportIdMap<egui::ViewportOutput>,
    ) -> Finished {
        let ppp = self.ppp();
        for (id, output) in viewports {
            let Some(child) = self.children.get_mut(id) else {
                continue;
            };
            for command in &output.commands {
                match command {
                    egui::ViewportCommand::Title(title) if child.title != *title => {
                        child.title = title.clone();
                        self.outbox.push(Command::Title {
                            key: child.key,
                            title: title.clone(),
                        });
                    }
                    egui::ViewportCommand::InnerSize(size) => {
                        let width = (size.x * ppp).round().clamp(1.0, 16384.0) as u32;
                        let height = (size.y * ppp).round().clamp(1.0, 16384.0) as u32;
                        self.outbox.push(Command::Resize {
                            key: child.key,
                            width,
                            height,
                        });
                    }
                    _ => {}
                }
            }
        }
        let gone: Vec<ViewportId> = self
            .children
            .iter()
            .filter(|(_, child)| !child.used)
            .map(|(id, _)| *id)
            .collect();
        let mut closed = Vec::new();
        for id in gone {
            if let Some(child) = self.children.remove(&id) {
                self.by_key.remove(&child.key);
                self.outbox.push(Command::Close { key: child.key });
                closed.push(child.key);
            }
        }
        Finished {
            textures: std::mem::take(&mut self.textures),
            frames: std::mem::take(&mut self.frames)
                .into_values()
                .filter(|frame| !closed.contains(&frame.key))
                .collect(),
            closed,
            urls: std::mem::take(&mut self.urls),
        }
    }

    /// Is `key` a window the app still shows (or just opened)?
    pub fn knows(&self, key: u32) -> bool {
        self.by_key.contains_key(&key)
    }

    pub fn take_outbox(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.outbox)
    }

    fn child_mut(&mut self, key: u32) -> Option<&mut Child> {
        let id = self.by_key.get(&key)?;
        self.children.get_mut(id)
    }

    /// The window's surface exists (first time, or again after a background trip).
    pub fn attach(&mut self, key: u32, width: u32, height: u32) {
        let mut scratch = Vec::new();
        if let Some(child) = self.child_mut(key) {
            child.phase = Phase::Attached;
            if width > 0 && height > 0 {
                child.size_px = (width, height);
            }
            child.input.cancel(Pos2::ZERO, &mut scratch);
            child.events.append(&mut scratch);
        }
    }

    pub fn resize(&mut self, key: u32, width: u32, height: u32) {
        if let Some(child) = self.child_mut(key)
            && width > 0
            && height > 0
        {
            child.size_px = (width, height);
        }
    }

    /// Surface destroyed: keep the window registered, the user did not close it.
    pub fn detach(&mut self, key: u32) {
        let mut scratch = Vec::new();
        if let Some(child) = self.child_mut(key) {
            child.phase = Phase::Detached;
            child.input.cancel(Pos2::ZERO, &mut scratch);
        }
    }

    #[cfg(test)]
    pub fn attached(&self, key: u32) -> bool {
        self.by_key
            .get(&key)
            .and_then(|id| self.children.get(id))
            .is_some_and(|child| child.phase == Phase::Attached)
    }

    /// The user asked to close the window (system close button, or open failed).
    pub fn request_close(&mut self, key: u32) {
        if let Some(child) = self.child_mut(key) {
            child.close_requested = true;
        }
    }

    /// Focus lost / pointer cancelled in this window.
    pub fn cancel(&mut self, key: u32) {
        if let Some(child) = self.child_mut(key) {
            let mut events = Vec::new();
            child.input.cancel(Pos2::ZERO, &mut events);
            child.events.append(&mut events);
        }
    }

    pub fn push_input(&mut self, key: u32, value: input::Input, pixels_per_point: f32) {
        if let Some(child) = self.child_mut(key) {
            let mut events = Vec::new();
            child.input.push(value, pixels_per_point, &mut events);
            child.events.append(&mut events);
        }
    }
}

/// Result of [`Registry::finish_frame`].
pub struct Finished {
    pub textures: egui::TexturesDelta,
    pub frames: Vec<ChildFrame>,
    pub closed: Vec<u32>,
    pub urls: Vec<String>,
}

/// Install the immediate-viewport renderer on the current (worker) thread.
pub fn install(registry: Rc<RefCell<Registry>>) {
    egui::Context::set_immediate_viewport_renderer(move |ctx, viewport| {
        let egui::ImmediateViewport {
            ids,
            builder,
            mut viewport_ui_cb,
        } = viewport;
        let raw = {
            let mut registry = registry.borrow_mut();
            registry.depth += 1;
            registry.prepare(ids.this, ids.parent, &builder)
        };
        // The registry is not borrowed while egui runs the nested pass.
        let output = ctx.run_ui(raw, |ui| viewport_ui_cb(ui));
        let mut registry = registry.borrow_mut();
        registry.depth = registry.depth.saturating_sub(1);
        registry.collect(ids.this, output);
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/viewports.rs"]
mod tests;
