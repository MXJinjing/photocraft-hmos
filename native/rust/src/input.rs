use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use photocraft_ui_egui::stylus::{PenSample, PointerSource, StylusFeed, StylusInput};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Default)]
pub struct Contact {
    pub id: u64,
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize)]
pub struct StylusPacket {
    version: u32,
    #[serde(flatten)]
    input: StylusInput,
}

#[derive(Deserialize, Default)]
pub struct Input {
    pub kind: String,
    #[serde(default)]
    pub points: Vec<Contact>,
    #[serde(default)]
    pub zoom: f32,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
    #[serde(default)]
    pub action: i32,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub button: i32,
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub source: PointerSource,
    #[serde(default)]
    pub sample: Option<PenSample>,
    #[serde(default)]
    pub packet: Option<StylusPacket>,
    #[serde(default)]
    pub focus: String,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub start: usize,
    #[serde(default)]
    pub end: usize,
}
#[derive(Default)]
pub struct State {
    pressed: Option<PointerButton>,
    touches: BTreeMap<u64, Pos2>,
    touch_pointer: Option<u64>,
    pen_down: bool,
    ignore_fingers: bool,
    axis_zoom: Option<f32>,
    pub modifiers: Modifiers,
    pub feed: StylusFeed,
}
impl State {
    fn clear_touches(&mut self, events: &mut Vec<Event>) {
        for (id, pos) in std::mem::take(&mut self.touches) {
            events.push(Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(id),
                phase: egui::TouchPhase::Cancel,
                pos,
                force: None,
            });
        }
        self.touch_pointer = None;
    }
    pub fn cancel(&mut self, pos: Pos2, events: &mut Vec<Event>) {
        self.clear_touches(events);
        self.pen_down = false;
        self.ignore_fingers = false;
        self.axis_zoom = None;
        if let Some(button) = self.pressed.take() {
            events.push(Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: self.modifiers,
            });
        }
        events.push(Event::PointerGone);
        self.feed.report(PointerSource::Mouse, None);
        self.modifiers = Modifiers::NONE;
        events.push(Event::ModifiersChanged(self.modifiers));
    }
    pub fn push(&mut self, input: Input, scale: f32, events: &mut Vec<Event>) {
        // 平台坐标是物理像素，egui 事件使用点；同一输入状态也负责跟踪按键与接触释放。
        if !input.x.is_finite() || !input.y.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return;
        }
        // XComponent pointer events do not carry keyboard modifiers. Retain the
        // most recent key state until another key event or focus cancellation.
        if input.kind == "key" {
            let old = self.modifiers;
            self.modifiers = Modifiers {
                ctrl: input.ctrl,
                shift: input.shift,
                alt: input.alt,
                command: input.ctrl,
                ..Default::default()
            };
            if old != self.modifiers {
                events.push(Event::ModifiersChanged(self.modifiers));
            }
        }
        let pos = Pos2::new(input.x / scale, input.y / scale);
        match input.kind.as_str() {
            "stylus" => {
                if let Some(packet) = input.packet
                    && packet.version == 1
                {
                    self.feed.submit(packet.input);
                }
            }
            "touch" => {
                // A snapshot carries every pressed finger, including unchanged contacts.
                // Keep palms suppressed until all fingers lift after a pen stroke.
                if self.pen_down || self.ignore_fingers {
                    self.ignore_fingers = !input.points.is_empty();
                    return;
                }
                if input.points.len() > 10
                    || input
                        .points
                        .iter()
                        .any(|p| !p.x.is_finite() || !p.y.is_finite())
                {
                    return;
                }
                let next: BTreeMap<_, _> = input
                    .points
                    .into_iter()
                    .map(|p| (p.id, Pos2::new(p.x / scale, p.y / scale)))
                    .collect();
                self.feed.report(PointerSource::Touch, None);
                for (&id, &pos) in &self.touches {
                    if !next.contains_key(&id) {
                        events.push(Event::Touch {
                            device_id: egui::TouchDeviceId(0),
                            id: egui::TouchId(id),
                            phase: egui::TouchPhase::End,
                            pos,
                            force: None,
                        });
                        if self.touch_pointer == Some(id) {
                            if let Some(button) = self.pressed.take() {
                                events.push(Event::PointerButton {
                                    pos,
                                    button,
                                    pressed: false,
                                    modifiers: self.modifiers,
                                });
                            }
                            events.push(Event::PointerGone);
                            self.touch_pointer = None;
                        }
                    }
                }
                let first = self.touches.is_empty();
                for (&id, &pos) in &next {
                    let phase = if self.touches.contains_key(&id) {
                        egui::TouchPhase::Move
                    } else {
                        egui::TouchPhase::Start
                    };
                    events.push(Event::Touch {
                        device_id: egui::TouchDeviceId(0),
                        id: egui::TouchId(id),
                        phase,
                        pos,
                        force: None,
                    });
                    if first && self.touch_pointer.is_none() && self.pressed.is_none() {
                        self.touch_pointer = Some(id);
                        self.pressed = Some(PointerButton::Primary);
                        events.push(Event::PointerMoved(pos));
                        events.push(Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed: true,
                            modifiers: self.modifiers,
                        });
                    } else if self.touch_pointer == Some(id) {
                        events.push(Event::PointerMoved(pos));
                    }
                }
                self.touches = next;
            }
            "axis" => {
                // ArkUI pinch scales are cumulative, whereas egui Zoom expects a ratio.
                if input.action == 1 {
                    self.axis_zoom = None;
                }
                self.feed.report(PointerSource::Mouse, None);
                events.push(Event::PointerMoved(pos));
                if input.zoom.is_finite() && input.zoom > 0.0 {
                    let ratio = input.zoom / self.axis_zoom.unwrap_or(1.0);
                    if ratio.is_finite() && ratio > 0.0 && ratio != 1.0 {
                        events.push(Event::Zoom(ratio));
                    }
                    self.axis_zoom = Some(input.zoom);
                } else if input.dx.is_finite()
                    && input.dy.is_finite()
                    && (input.dx != 0.0 || input.dy != 0.0)
                {
                    events.push(Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(input.dx / scale, input.dy / scale),
                        modifiers: self.modifiers,
                    });
                }
                if input.action == 3 || input.action == 4 {
                    self.axis_zoom = None;
                }
            }
            "pointer" => {
                if input.action == 3 {
                    self.cancel(pos, events);
                    return;
                }
                if input.source == PointerSource::Pen && input.action == 0 {
                    self.ignore_fingers = !self.touches.is_empty();
                    self.clear_touches(events);
                    if let Some(button) = self.pressed.take() {
                        events.push(Event::PointerButton {
                            pos,
                            button,
                            pressed: false,
                            modifiers: self.modifiers,
                        });
                    }
                    self.pen_down = true;
                } else if self.pen_down && input.source != PointerSource::Pen {
                    return;
                }
                if input.source == PointerSource::Pen && input.action == 1 {
                    self.pen_down = false;
                }
                self.feed.report(input.source, input.sample);
                events.push(Event::PointerMoved(pos));
                let button = match input.button {
                    1 => PointerButton::Secondary,
                    2 => PointerButton::Middle,
                    _ => PointerButton::Primary,
                };
                match input.action {
                    0 if self.pressed.is_none() => {
                        self.pressed = Some(button);
                        events.push(Event::PointerButton {
                            pos,
                            button,
                            pressed: true,
                            modifiers: self.modifiers,
                        });
                    }
                    1 => {
                        if let Some(button) = self.pressed.take() {
                            events.push(Event::PointerButton {
                                pos,
                                button,
                                pressed: false,
                                modifiers: self.modifiers,
                            });
                        }
                    }
                    _ => {}
                }
            }
            "wheel" => events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(input.x, input.y),
                modifiers: self.modifiers,
            }),
            "key" => {
                let pressed = input.action == 0;
                if let Some(key) = key(input.code) {
                    events.push(Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed,
                        repeat: false,
                        modifiers: self.modifiers,
                    });
                }
                if pressed
                    && !input.ctrl
                    && !input.alt
                    && !input.text.is_empty()
                    && !input.text.chars().any(char::is_control)
                {
                    events.push(Event::Text(input.text));
                }
            }
            "blur" => self.cancel(pos, events),
            _ => {}
        }
    }
}
fn key(code: i32) -> Option<Key> {
    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    const DIGITS: [Key; 10] = [
        Key::Num0,
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    if (2017..=2042).contains(&code) {
        return LETTERS.get((code - 2017) as usize).copied();
    }
    if (2000..=2009).contains(&code) {
        return DIGITS.get((code - 2000) as usize).copied();
    }
    Some(match code {
        2012 => Key::ArrowUp,
        2013 => Key::ArrowDown,
        2014 => Key::ArrowLeft,
        2015 => Key::ArrowRight,
        2049 => Key::Tab,
        2050 => Key::Space,
        2054 => Key::Enter,
        2055 => Key::Backspace,
        2068 => Key::PageUp,
        2069 => Key::PageDown,
        2070 => Key::Escape,
        2071 => Key::Delete,
        2081 => Key::Home,
        2082 => Key::End,
        _ => return None,
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "../tests/unit/input.rs"]
mod tests;
