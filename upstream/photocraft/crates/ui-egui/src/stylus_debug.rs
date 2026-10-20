//! Session-only input monitor. Reads the feed without consuming samples or changing documents.
use egui::{Align2, FontId, Pos2, Sense, Stroke, vec2};
use serde::{Deserialize, Serialize};

use crate::PhotocraftApp;
use crate::stylus::{PenSample, PointerSource, StylusDiagnostics};
use crate::theme::Tokens;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugUi {
    pub open: bool,
    pub suppress_actions: bool,
    #[serde(skip)]
    baseline: [u64; 3],
    #[serde(skip)]
    observed: [u64; 2],
    #[serde(skip)]
    flashes: [Option<f64>; 2],
    #[serde(skip)]
    ink: Vec<Option<[f32; 3]>>,
    #[serde(skip)]
    drawing: bool,
}

impl Default for DebugUi {
    fn default() -> Self {
        Self { open: false, suppress_actions: true, baseline: [0; 3], observed: [0; 2], flashes: [None; 2], ink: Vec::new(), drawing: false }
    }
}

impl DebugUi {
    pub fn set_open(&mut self, open: bool, d: &StylusDiagnostics) {
        if open && !self.open {
            self.reset(d);
        }
        self.open = open;
    }

    fn reset(&mut self, d: &StylusDiagnostics) {
        self.baseline = [d.samples, d.double_taps, d.squeezes];
        self.observed = [d.double_taps, d.squeezes];
        self.flashes = [None; 2];
        self.ink.clear();
        self.drawing = false;
    }

    fn observe(&mut self, d: &StylusDiagnostics, now: f64) {
        for ((previous, flash), count) in self.observed.iter_mut().zip(&mut self.flashes).zip([d.double_taps, d.squeezes]) {
            if *previous != count {
                *flash = Some(now);
                *previous = count;
            }
        }
    }
}

/// W3C tilt axes projected into screen coordinates (+x right, +y down).
/// A vertical pen has no bearing. This is not barrel rotation or a nib's chisel angle.
fn orientation(p: PenSample) -> (f32, Option<f32>) {
    let p = p.sanitized();
    let x = p.tilt_x.clamp(-89.999, 89.999).to_radians().tan();
    let y = p.tilt_y.clamp(-89.999, 89.999).to_radians().tan();
    let length = x.hypot(y);
    let bearing = (length > 0.0001).then(|| x.atan2(-y).to_degrees().rem_euclid(360.0));
    (length.atan().to_degrees(), bearing)
}

fn dial(ui: &mut egui::Ui, p: Option<PenSample>) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(200.0, 200.0), Sense::hover());
    let center = rect.center();
    let radius = 72.0;
    let painter = ui.painter();
    painter.circle_stroke(center, radius, Stroke::new(2.0, t.field_border));
    painter.circle_stroke(center, radius * 0.5, Stroke::new(1.0, t.separator));
    for (offset, text) in [(vec2(0.0, -89.0), "0°"), (vec2(89.0, 0.0), "90°"), (vec2(0.0, 89.0), "180°"), (vec2(-89.0, 0.0), "270°")] {
        painter.text(center + offset, Align2::CENTER_CENTER, text, FontId::proportional(11.0), t.text_dim);
    }
    painter.line_segment([center - vec2(radius, 0.0), center + vec2(radius, 0.0)], Stroke::new(1.0, t.separator));
    painter.line_segment([center - vec2(0.0, radius), center + vec2(0.0, radius)], Stroke::new(1.0, t.separator));
    if let Some(p) = p {
        let (inclination, bearing) = orientation(p);
        if let Some(bearing) = bearing {
            let a = bearing.to_radians();
            let direction = vec2(a.sin(), -a.cos());
            painter.arrow(center, direction * radius * (inclination / 90.0).clamp(0.18, 1.0), Stroke::new(3.0, t.accent));
        }
    }
    painter.circle_filled(center, 4.0, if p.is_some() { t.accent } else { t.text_faint });
}

fn gesture_row(ui: &mut egui::Ui, label: &str, count: u64, flash: Option<f64>, now: f64) {
    let t = Tokens::get(ui.ctx());
    let lit = flash.is_some_and(|at| (0.0..1.2).contains(&(now - at)));
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 5.0, if lit { t.online } else { t.text_faint });
        ui.label(tl!(label));
        ui.monospace(format!("× {count}"));
        if lit {
            ui.colored_label(t.online, tl!("Received"));
        }
    });
}

/// The safe test pad consumes its own pointer interactions, never canvas/engine commands.
fn test_pad(ui: &mut egui::Ui, state: &mut DebugUi, sample: Option<PenSample>, source: PointerSource) {
    let t = Tokens::get(ui.ctx());
    ui.label(tl!("Test area — does not change the document"));
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 100.0), Sense::drag());
    ui.painter().rect_filled(r, 6.0, t.field);
    let point = ui.input(|i| i.pointer.interact_pos());
    let down = response.is_pointer_button_down_on() && source == PointerSource::Pen && sample.is_some_and(|s| s.pressure > 0.0);
    if down {
        if let (Some(at), Some(sample)) = (point.filter(|p| r.contains(*p)), sample) {
            if state.ink.len() >= 2048 {
                state.ink.drain(..512);
            }
            let local = at - r.min;
            state.ink.push(Some([local.x, local.y, sample.pressure]));
            state.drawing = true;
        }
    } else if state.drawing {
        state.ink.push(None);
        state.drawing = false;
    }
    let mut previous: Option<Pos2> = None;
    let painter = ui.painter().with_clip_rect(r);
    for point in &state.ink {
        if let Some([x, y, pressure]) = point {
            let at = r.min + vec2(*x, *y);
            let width = 1.0 + pressure * 7.0;
            if let Some(prev) = previous {
                painter.line_segment([prev, at], Stroke::new(width, t.accent));
            }
            painter.circle_filled(at, width * 0.5, t.accent);
            previous = Some(at);
        } else {
            previous = None;
        }
    }
    if state.ink.is_empty() {
        painter.text(r.center(), Align2::CENTER_CENTER, tl!("Draw here with the pen"), FontId::proportional(14.0), t.text_dim);
    }
}

pub fn window(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let viewport = egui::ViewportId::from_hash_of("stylus-debug");
    if !app.ui.stylus_debug.open {
        app.stylus.feed.clear_hover(viewport);
        return;
    }
    // Opt into the same native immediate-viewport host as floating documents. Keep the
    // host's embedding preference for all other windows (and retain a web/test fallback).
    let embedded = ctx.embed_viewports();
    if app.native_doc_windows {
        ctx.set_embed_viewports(false);
    }
    let builder = egui::ViewportBuilder::default().with_title(tl!("Stylus Debug")).with_inner_size([600.0, 720.0]).with_min_inner_size([480.0, 300.0]);
    ctx.show_viewport_immediate(viewport, builder, |ui, class| {
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            app.stylus.feed.clear_hover(viewport);
            app.ui.stylus_debug.open = false;
            // The system close callback vetoes the first click so this viewport can remove its
            // native surface. Without another root frame, the host keeps the window alive until
            // the user clicks Close again.
            ui.ctx().request_repaint();
            return;
        }
        draw(app, ui, class == egui::ViewportClass::EmbeddedWindow);
    });
    ctx.set_embed_viewports(embedded);
}

fn draw(app: &mut PhotocraftApp, ui: &mut egui::Ui, embedded: bool) {
    let ctx = ui.ctx().clone();
    let d = app.stylus.feed.diagnostics();
    let device = app.stylus.feed.device();
    let source = app.stylus.feed.source();
    let current = app.stylus.feed.get();
    let now = ctx.input(|i| i.time);
    let hover_here = d.hovering && d.hover_viewport == Some(ctx.viewport_id());
    let contact_here = source == PointerSource::Pen
        && current.is_some_and(|s| s.pressure > 0.0)
        && d.contact_viewport == Some(ctx.viewport_id());
    let state = &mut app.ui.stylus_debug;
    state.observe(&d, now);
    let p = (d.samples > state.baseline[0]).then_some(d.last_pen).flatten();
    let mut open = state.open;
    let content = |ui: &mut egui::Ui| {
        ui.label(tl!("Use a real Pad and a pen that supports tilt, double-tap and squeeze."));
        ui.horizontal_wrapped(|ui| {
            ui.label(if device.connected { tl!("Connected") } else { tl!("No connection reported") });
            if let Some(name) = &device.name {
                ui.label(name);
            }
        });
        ui.horizontal(|ui| {
            dial(ui, p);
            ui.vertical(|ui| {
                ui.strong(tl!("Latest pen sample"));
                if let Some(p) = p {
                    ui.monospace(format!("Tilt X: {:+.1}°   Tilt Y: {:+.1}°", p.tilt_x, p.tilt_y));
                    let (inclination, bearing) = orientation(p);
                    ui.label(format!("{}: {inclination:.1}°", tl!("Tilt from vertical")));
                    ui.label(format!("{}: {}", tl!("Screen bearing"), bearing.map_or_else(|| "—".into(), |v| format!("{v:.1}°"))));
                    ui.add(egui::ProgressBar::new(p.pressure).text(format!("{}: {:.1}%", tl!("Pressure"), p.pressure * 100.0)));
                    ui.label(if p.eraser { tl!("Eraser end") } else { tl!("Pen tip") });
                } else {
                    ui.label(tl!("Waiting for pen input"));
                }
                ui.label(format!("{}: {}  ·  {:.1} Hz", tl!("Samples"), d.samples.saturating_sub(state.baseline[0]), d.sample_rate_hz));
                ui.label(format!("{}: {}°", tl!("Rotation"), d.display_rotation * 90));
                ui.label(if hover_here {
                    d.hover.map_or_else(|| tl!("Hovering").to_owned(), |[x, y]| format!("{}  X {x:.0}  Y {y:.0}", tl!("Hovering")))
                } else if contact_here {
                    d.contact.map_or_else(|| tl!("Pen contact").to_owned(), |[x, y]| format!("{}  X {x:.0}  Y {y:.0}", tl!("Pen contact")))
                } else {
                    tl!("No pen contact — values are retained").to_owned()
                });
            });
        });
        ui.small(tl!("Arrow: screen-projected tilt direction, not barrel rotation. Upright: no direction."));
        ui.small(tl!("Zero tilt may mean an upright pen or unavailable hardware data."));
        ui.small(tl!("HarmonyOS contact input does not currently report barrel rotation."));
        ui.separator();
        gesture_row(ui, "Double-tap pen body", d.double_taps.saturating_sub(state.baseline[1]), state.flashes[0], now);
        gesture_row(ui, "Squeeze pen body", d.squeezes.saturating_sub(state.baseline[2]), state.flashes[1], now);
        ui.checkbox(&mut state.suppress_actions, tl!("Test gestures only (do not run shortcuts)"));
        ui.small(tl!("No indicator means no event received, not necessarily unsupported hardware."));
        ui.separator();
        test_pad(ui, state, current, source);
        ui.horizontal(|ui| {
            if ui.button(tl!("Reset test")).clicked() {
                state.reset(&d);
            }
        });
        ui.small(tl!("Readings are before the pressure curve and remain available with tablet pressure off."));
    };
    if embedded {
        egui::Window::new(tl!("Stylus Debug"))
            .id(egui::Id::new("stylus-debug-embedded"))
            .open(&mut open)
            .default_size(vec2(560.0, 720.0))
            .min_width(480.0)
            .vscroll(true)
            .show(&ctx, content);
    } else {
        // System decoration owns the title/close button. Only egui content is drawn here.
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Tokens::get(&ctx).card).inner_margin(16.0)).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, content);
        });
    }
    state.open = open;
    // Only the open monitor polls; closing it stops the timer. A local UI clock drives flashes.
    if open {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
#[path = "stylus_debug_tests.rs"]
mod tests;
