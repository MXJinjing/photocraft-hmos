use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct Input {
    pub kind: String,
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
}
#[derive(Default)]
pub struct State {
    pressed: Option<PointerButton>,
    pub modifiers: Modifiers,
}
impl State {
    pub fn cancel(&mut self, pos: Pos2, events: &mut Vec<Event>) {
        if let Some(button) = self.pressed.take() {
            events.push(Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: self.modifiers,
            });
        }
        events.push(Event::PointerGone);
        self.modifiers = Modifiers::NONE;
        events.push(Event::ModifiersChanged(self.modifiers));
    }
    pub fn push(&mut self, input: Input, scale: f32, events: &mut Vec<Event>) {
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
            "pointer" => {
                if input.action == 3 {
                    self.cancel(pos, events);
                    return;
                }
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
mod tests {
    use super::*;
    #[test]
    fn cancel_releases_pointer_and_scales_coordinates() {
        let mut state = State::default();
        let mut events = vec![];
        state.push(
            Input {
                kind: "pointer".into(),
                x: 200.,
                y: 100.,
                ..Default::default()
            },
            2.,
            &mut events,
        );
        assert!(matches!(events.first(),Some(Event::PointerMoved(p)) if *p==Pos2::new(100.,50.)));
        state.cancel(Pos2::ZERO, &mut events);
        assert!(matches!(
            events.get(2),
            Some(Event::PointerButton { pressed: false, .. })
        ));
        assert!(state.pressed.is_none());
    }
    #[test]
    fn pointer_and_wheel_preserve_held_modifiers_until_focus_loss() {
        let mut state = State::default();
        let mut events = vec![];
        state.push(
            Input {
                kind: "key".into(),
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            1.,
            &mut events,
        );
        state.push(
            Input {
                kind: "pointer".into(),
                ..Default::default()
            },
            1.,
            &mut events,
        );
        assert!(
            matches!(events.last(), Some(Event::PointerButton { modifiers, .. }) if modifiers.ctrl && modifiers.shift)
        );
        state.push(
            Input {
                kind: "wheel".into(),
                y: 10.,
                ..Default::default()
            },
            1.,
            &mut events,
        );
        assert!(
            matches!(events.last(), Some(Event::MouseWheel { modifiers, .. }) if modifiers.ctrl && modifiers.shift)
        );
        state.push(
            Input {
                kind: "blur".into(),
                ..Default::default()
            },
            1.,
            &mut events,
        );
        assert_eq!(state.modifiers, Modifiers::NONE);
    }
    #[test]
    fn shortcut_does_not_insert_text_and_bad_numbers_are_ignored() {
        let mut state = State::default();
        let mut events = vec![];
        state.push(
            Input {
                kind: "key".into(),
                code: 2035,
                text: "s".into(),
                ctrl: true,
                ..Default::default()
            },
            1.,
            &mut events,
        );
        assert_eq!(events.len(), 2);
        state.push(
            Input {
                kind: "pointer".into(),
                x: f32::NAN,
                ..Default::default()
            },
            1.,
            &mut events,
        );
        assert_eq!(events.len(), 2);
    }
}
