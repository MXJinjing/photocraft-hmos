//! Stylus input: real pen pressure (and tilt / barrel rotation where the platform reports them)
//! for the painting tools. A mouse always paints at pressure 1 with no tilt.
//!
//! Sources, by platform (eframe 0.36.2, egui-winit 0.36.2, winit 0.30.13):
//!
//! - **Windows**: winit turns `WM_POINTER` pen and touch input into `WindowEvent::Touch` with a
//!   normalised force (`POINTER_PEN_INFO::pressure / 1024`); egui-winit forwards it as
//!   `egui::Event::Touch { force }` alongside the emulated pointer. [`Stylus::update`] reads it.
//!   winit drops the pen's tilt and rotation, so those stay 0.
//! - **Web**: eframe forwards touch force but not pen pointer events, so the web runner listens
//!   to `pointerdown`/`pointermove` itself and writes `pressure`, `tiltX`, `tiltY`, `twist` and
//!   the eraser button of `pointerType == "pen"` events into the [`StylusFeed`], plus the
//!   `pointerType` so fingers can be told from the pen.
//! - **macOS**: winit 0.30 drops `NSEvent` tablet data, so the desktop app installs an AppKit
//!   local event monitor (the `photocraft-tablet` crate) that writes pressure, tilt, rotation and
//!   the eraser end into the [`StylusFeed`] before winit handles each event.
//! - **Linux X11**: the desktop app reads XInput2 raw valuator events on its own X connection
//!   (`photocraft-tablet`, x11rb) and writes them into the [`StylusFeed`].
//! - **Linux Wayland**: compositors give a pen only to clients that bind `zwp_tablet_v2`, which
//!   winit doesn't (it would have to share winit's connection), so with a pen attached the
//!   desktop app opens its window through Xwayland, which reports tablet valuators to the X11
//!   reader. `PHOTOCRAFT_NATIVE_WAYLAND=1` keeps native Wayland, where the pen has no input.
//!
//! Preferences › Tools › Use Tablet Pressure off makes a pen paint like a mouse. Flipping the pen
//! to its eraser end selects the Eraser tool and flipping back restores the previous tool, as in
//! Photoshop. Platform adapters submit [`StylusInput`] to the shared feed. The shell consumes
//! normalised double-tap and long-press actions without knowing which SDK produced them.
//! The versioned host transport is documented in `docs/stylus-input.md`.
//!
//! Automation simulates a pen with `ui.pointer` events carrying `pressure`, `tiltX`, `tiltY`
//! and `rotation`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

/// One stylus reading. Tilt is in degrees (-90..90, W3C Pointer Events convention), rotation in
/// degrees 0..360.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PenSample {
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub rotation: f32,
    /// The pen's eraser end is in use.
    pub eraser: bool,
}

impl Default for PenSample {
    fn default() -> Self {
        Self { pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, rotation: 0.0, eraser: false }
    }
}

impl PenSample {
    /// Clamp to the documented ranges (and replace non-finite values).
    pub fn sanitized(self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Self {
            pressure: f(self.pressure, 0.0, 1.0, 1.0),
            tilt_x: f(self.tilt_x, -90.0, 90.0, 0.0),
            tilt_y: f(self.tilt_y, -90.0, 90.0, 0.0),
            // `rem_euclid` rounds tiny negative angles up to exactly 360.
            rotation: if self.rotation.is_finite() { self.rotation.rem_euclid(360.0) % 360.0 } else { 0.0 },
            eraser: self.eraser,
        }
    }
}

const FEED_QUEUE: usize = 1024;

/// What last wrote the pointer slot: a mouse, a pen, or a finger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PointerSource {
    #[default]
    Mouse,
    Pen,
    Touch,
}

impl PointerSource {
    /// W3C `pointerType`: `"pen"`, `"touch"`, `"mouse"`, or anything else.
    pub fn from_pointer_type(ty: &str) -> Self {
        match ty {
            "pen" => Self::Pen,
            "touch" => Self::Touch,
            _ => Self::Mouse,
        }
    }
}

/// Platform-independent pen actions. Adapters translate physical gestures (for example a
/// squeeze or a held side button) into these actions; tool preferences decide what they do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PenGesture {
    DoubleTap,
    LongPress,
}

/// The common input contract for browser, native tablet and host adapters.
/// All timestamps use milliseconds on the same clock (web adapters use Unix epoch time).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StylusInput {
    Pointer { source: PointerSource, sample: Option<PenSample> },
    BarrelButton { pressed: bool, timestamp_ms: f64 },
    Gesture { gesture: PenGesture, timestamp_ms: f64 },
    Connection { connected: bool },
}

/// Barrel-button timing. A short second press within [`DOUBLE_MS`] is a double-tap; holding for
/// [`LONG_MS`] is a long-press. A long-press does not also count as a tap.
#[derive(Clone, Debug, Default)]
pub struct BarrelWatch {
    down_at: Option<f64>,
    last_tap_up: Option<f64>,
    long_sent: bool,
}

impl BarrelWatch {
    /// Two short barrel presses this close (ms) are one double-tap.
    pub const DOUBLE_MS: f64 = 400.0;
    /// Holding the barrel this long (ms) is a long-press.
    pub const LONG_MS: f64 = 450.0;

    /// `down` is the barrel button this sample. `now` is a monotonic millisecond clock.
    pub fn update(&mut self, down: bool, now: f64) -> Option<PenGesture> {
        if !now.is_finite() {
            return None;
        }
        if down {
            let started = *self.down_at.get_or_insert(now);
            if !self.long_sent && now - started >= Self::LONG_MS {
                self.long_sent = true;
                self.last_tap_up = None;
                return Some(PenGesture::LongPress);
            }
            return None;
        }
        let started = self.down_at.take()?;
        let was_long = std::mem::take(&mut self.long_sent);
        if was_long || now - started >= Self::LONG_MS {
            self.last_tap_up = None;
            return None;
        }
        if let Some(prev) = self.last_tap_up.take()
            && now - prev <= Self::DOUBLE_MS
        {
            return Some(PenGesture::DoubleTap);
        }
        self.last_tap_up = Some(now);
        None
    }
}

#[derive(Debug)]
struct Slot {
    sample: Option<PenSample>,
    source: PointerSource,
    gestures: Vec<PenGesture>,
    barrel: BarrelWatch,
    last_gesture: Option<(PenGesture, f64)>,
    /// Attachment reported by an adapter, distinct from contact with the screen.
    connected: bool,
    ever_connected: bool,
    queue: VecDeque<PenSample>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            sample: None,
            source: PointerSource::Mouse,
            gestures: Vec::new(),
            barrel: BarrelWatch::default(),
            last_gesture: None,
            connected: false,
            ever_connected: false,
            queue: VecDeque::new(),
        }
    }
}

fn enqueue_gesture(slot: &mut Slot, gesture: PenGesture, now: f64) {
    if let Some((prev, at)) = slot.last_gesture
        && prev == gesture
        && now.is_finite()
        && at.is_finite()
        && (now - at).abs() < 350.0
    {
        return;
    }
    slot.last_gesture = Some((gesture, now));
    slot.gestures.push(gesture);
    let overflow = slot.gestures.len().saturating_sub(8);
    if overflow > 0 {
        slot.gestures.drain(0..overflow);
    }
}

/// Shared slot a platform backend writes the current pen sample into (`None` = no pen down).
/// Cloning shares the slot.
#[derive(Clone, Debug, Default)]
pub struct StylusFeed(Arc<Mutex<Slot>>);

impl StylusFeed {
    /// Submit normalised input from any adapter. True requests a UI repaint; false means the
    /// input was unchanged, a side-button transition has no action yet, or its time is invalid.
    pub fn submit(&self, input: StylusInput) -> bool {
        match input {
            StylusInput::Pointer { source, sample } => {
                self.report(source, sample);
                true
            }
            StylusInput::BarrelButton { pressed, timestamp_ms } => self.report_barrel(pressed, timestamp_ms).is_some(),
            StylusInput::Gesture { gesture, timestamp_ms } => {
                if !timestamp_ms.is_finite() {
                    return false;
                }
                self.push_gesture(gesture, timestamp_ms);
                true
            }
            StylusInput::Connection { connected } => self.set_connected(connected),
        }
    }

    /// Convenience for native tablet monitors that only provide pen samples.
    pub fn set(&self, s: Option<PenSample>) {
        let source = if s.is_some() { PointerSource::Pen } else { PointerSource::Mouse };
        self.submit(StylusInput::Pointer { source, sample: s });
    }
    pub fn get(&self) -> Option<PenSample> {
        self.0.lock().ok().and_then(|g| g.sample)
    }
    pub fn source(&self) -> PointerSource {
        self.0.lock().ok().map_or(PointerSource::Mouse, |g| g.source)
    }
    /// Web Pointer Events: record the device and, for a pen contact, its sample.
    pub fn report(&self, source: PointerSource, sample: Option<PenSample>) {
        if let Ok(mut g) = self.0.lock() {
            g.source = source;
            g.sample = if source == PointerSource::Pen { sample.map(PenSample::sanitized) } else { None };
            if let Some(sample) = g.sample {
                if g.queue.len() >= FEED_QUEUE {
                    g.queue.pop_front();
                }
                g.queue.push_back(sample);
            }
        }
    }
    /// Barrel button from a pen Pointer Event. Returns the gesture just recognised, if any.
    pub fn report_barrel(&self, down: bool, now_ms: f64) -> Option<PenGesture> {
        let Ok(mut g) = self.0.lock() else { return None };
        let gesture = g.barrel.update(down, now_ms)?;
        enqueue_gesture(&mut g, gesture, now_ms);
        Some(gesture)
    }
    /// An adapter named the action directly. Repeats within 350 ms are dropped so the host
    /// event and a browser side-button fallback cannot run the action twice.
    pub fn push_gesture(&self, gesture: PenGesture, now_ms: f64) {
        if let Ok(mut g) = self.0.lock() {
            enqueue_gesture(&mut g, gesture, now_ms);
        }
    }
    /// Whether a stylus is attached, as reported by the platform adapter.
    pub fn set_connected(&self, on: bool) -> bool {
        let Ok(mut g) = self.0.lock() else { return false };
        let changed = g.connected != on;
        g.connected = on;
        g.ever_connected |= on;
        changed
    }
    /// A stylus is attached right now. False until the platform says otherwise.
    pub fn connected(&self) -> bool {
        self.0.lock().is_ok_and(|g| g.connected)
    }
    /// Keep stylus settings available after the first reported connection in this app lifetime.
    pub fn ever_connected(&self) -> bool {
        self.0.lock().is_ok_and(|g| g.ever_connected)
    }
    /// Gestures queued since the last take, oldest first.
    pub fn take_gestures(&self) -> Vec<PenGesture> {
        self.0.lock().ok().map(|mut g| std::mem::take(&mut g.gestures)).unwrap_or_default()
    }

    /// Samples since the last canvas frame, preserving each move's pressure.
    pub fn drain(&self) -> Vec<PenSample> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).queue.drain(..).collect()
    }
}

/// Per-app stylus state.
#[derive(Clone, Debug)]
pub struct Stylus {
    /// Pen samples pushed by the platform (desktop tablet monitor, web runner) or by automation.
    pub feed: StylusFeed,
    /// Preferences › Tools › Use Tablet Pressure: off, a pen paints like a mouse.
    pub use_pressure: bool,
    /// The pen end last seen (`Some(true)` = eraser), for the eraser tool switch.
    end: Option<bool>,
    /// The tool to restore when the pen tip comes back after the eraser end switched tools.
    pub(crate) tool_before_eraser: Option<crate::state::Tool>,
    /// Tool before the latest change, for double-tap › Previous tool.
    previous_tool: Option<crate::state::Tool>,
    /// Last tool [`Self::note_tool`] saw, so a change can be told from a repeat.
    tool_seen: Option<crate::state::Tool>,
    /// Tool to restore when double-tap swaps back from the Eraser.
    double_tap_restore: Option<crate::state::Tool>,
    /// Last pen contact in document coordinates. The menu uses the lift point after a stroke.
    pub(crate) last_pen_point: Option<[f64; 2]>,
    /// Force of the touch/pen contact currently down (egui `Event::Touch`).
    touch: Option<f32>,
    /// The contact ended this frame: keep its force for this frame's last tool events, clear next frame.
    lifted: bool,
    /// This frame's pen samples, oldest first (the feed's queue and the touch forces).
    frame: Vec<PenSample>,
    /// The last pen sample of an earlier frame: where this frame's interpolation starts.
    prev: Option<PenSample>,
    /// The sample [`Self::select`] interpolated for the pointer move being processed.
    current: Option<PenSample>,
    /// The egui frame [`Self::update_for_frame`] last ran in.
    updated_frame: Option<u64>,
    /// Tilt X, tilt Y, rotation of each point of the current drag (parallel to its points).
    pub(crate) stroke: Vec<[f32; 3]>,
    /// Time in milliseconds of each point of the current drag (parallel to its points), from
    /// [`Stylus::clock_ms`]. Speed spacing, airbrush build-up and smoothing catch-up use it.
    pub(crate) times: Vec<f64>,
    /// Time of the tool event being processed, in milliseconds. The canvas spreads a frame's
    /// samples between the previous and the current frame time (egui's events carry none).
    pub(crate) clock_ms: f64,
}

impl Default for Stylus {
    fn default() -> Self {
        Self {
            feed: StylusFeed::default(),
            use_pressure: true,
            end: None,
            tool_before_eraser: None,
            previous_tool: None,
            tool_seen: None,
            double_tap_restore: None,
            last_pen_point: None,
            touch: None,
            lifted: false,
            frame: Vec::new(),
            prev: None,
            current: None,
            updated_frame: None,
            stroke: Vec::new(),
            times: Vec::new(),
            clock_ms: 0.0,
        }
    }
}

impl Stylus {
    /// [`Self::update`] once per egui frame: the canvas runs once per view, and egui can run a
    /// frame in several passes; a second update would drain an empty queue and lose the frame's
    /// pen samples.
    pub fn update_for_frame(&mut self, frame: u64, events: &[egui::Event]) {
        if self.updated_frame.replace(frame) != Some(frame) {
            self.update(events);
        }
    }

    /// Track touch/pen contacts from this frame's input events.
    pub fn update(&mut self, events: &[egui::Event]) {
        if std::mem::take(&mut self.lifted) {
            self.touch = None;
        }
        if let Some(last) = self.frame.last() {
            self.prev = Some(*last);
        }
        self.frame = self.feed.drain();
        self.current = None;
        for e in events {
            if let egui::Event::Touch { phase, force, .. } = e {
                match phase {
                    egui::TouchPhase::Start | egui::TouchPhase::Move => {
                        if let Some(f) = force.filter(|f| f.is_finite()) {
                            self.touch = Some(f.clamp(0.0, 1.0));
                            self.frame.push(PenSample { pressure: f.clamp(0.0, 1.0), ..Default::default() });
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => self.lifted = true,
                }
            }
        }
    }

    /// The current pen sample, `None` for a mouse, a finger, or any pen while Use Tablet Pressure
    /// is off. Fingers never carry pressure: the brush uses the full tip size.
    pub fn sample(&self) -> Option<PenSample> {
        if !self.use_pressure || self.is_finger() {
            return None;
        }
        self.current.or_else(|| self.feed.get()).or(self.touch.map(|pressure| PenSample { pressure, ..Default::default() }))
    }

    /// Choose the pen sample for move `k` of the `n` pointer moves this frame delivered: the pen
    /// reports at its own rate, so each move takes the samples interpolated at its share of the
    /// frame (from the previous frame's last sample), instead of all sharing the latest one. A
    /// mouse, or a frame without pen samples, keeps the current sample.
    pub(crate) fn select(&mut self, k: usize, n: usize) {
        self.current = None;
        if !self.use_pressure || n == 0 || self.frame.is_empty() || self.sample().is_none() {
            return;
        }
        let m = self.frame.len();
        let start = self.prev.or(self.frame.first().copied()).unwrap_or_default();
        let at = |i: usize| if i == 0 { start } else { self.frame.get(i - 1).copied().unwrap_or(start) };
        let pos = (k + 1).min(n) as f32 / n as f32 * m as f32;
        let i = pos.floor() as usize;
        let (a, b, t) = (at(i), at((i + 1).min(m)), pos - pos.floor());
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        // Rotation takes the shorter way round.
        let dr = (b.rotation - a.rotation + 540.0).rem_euclid(360.0) - 180.0;
        self.current = Some(
            PenSample {
                pressure: lerp(a.pressure, b.pressure),
                tilt_x: lerp(a.tilt_x, b.tilt_x),
                tilt_y: lerp(a.tilt_y, b.tilt_y),
                rotation: a.rotation + dr * t,
                eraser: b.eraser,
            }
            .sanitized(),
        );
    }

    /// Back to the current sample after a frame's moves.
    pub(crate) fn clear_selection(&mut self) {
        self.current = None;
    }

    /// Last Pointer Event was a finger (`pointerType == "touch"`).
    pub fn is_finger(&self) -> bool {
        self.feed.source() == PointerSource::Touch
    }

    /// Last Pointer Event was a pen (`pointerType == "pen"`), including hover.
    pub fn is_pen(&self) -> bool {
        self.feed.source() == PointerSource::Pen
    }

    /// Did the pen just flip to its eraser end (`Some(true)`) or back to its tip (`Some(false)`)?
    /// Each flip is reported once; a mouse in between changes nothing.
    pub(crate) fn take_end_flip(&mut self) -> Option<bool> {
        let eraser = self.feed.get()?.eraser;
        let flipped = self.end.map_or(eraser, |e| e != eraser);
        self.end = Some(eraser);
        flipped.then_some(eraser)
    }

    /// Remember a pen press or lift for positioning the stylus context menu.
    pub fn note_pen_point(&mut self, at: [f64; 2]) {
        if at.iter().all(|v| v.is_finite()) {
            self.last_pen_point = Some(at);
        }
    }

    /// Remember a tool change so double-tap can return to the previous tool.
    pub fn note_tool(&mut self, current: crate::state::Tool) {
        if self.tool_seen == Some(current) {
            return;
        }
        if self.tool_seen.is_some() {
            self.previous_tool = self.tool_seen;
        }
        self.tool_seen = Some(current);
    }

    /// Double-tap › current tool and Eraser: swap, then swap back to the tool double-tap left.
    pub fn toggle_eraser_tool(&mut self, tool: &mut crate::state::Tool) {
        use crate::state::Tool;
        if *tool == Tool::Eraser {
            let back = self.double_tap_restore.take().or(self.previous_tool).filter(|t| *t != Tool::Eraser);
            if let Some(back) = back {
                self.previous_tool = Some(Tool::Eraser);
                self.tool_seen = Some(back);
                *tool = back;
            }
            return;
        }
        self.double_tap_restore = Some(*tool);
        self.previous_tool = Some(*tool);
        self.tool_seen = Some(Tool::Eraser);
        *tool = Tool::Eraser;
    }

    /// Double-tap › previous tool: swap with the tool used just before this one.
    pub fn switch_previous_tool(&mut self, tool: &mut crate::state::Tool) {
        let Some(prev) = self.previous_tool else { return };
        if prev == *tool {
            return;
        }
        let current = *tool;
        self.previous_tool = Some(current);
        self.tool_seen = Some(prev);
        *tool = prev;
    }

    /// Switch to the Eraser when the pen's eraser end comes in, and back to the previous tool when
    /// the tip does (Photoshop). Not during a drag. Returns whether the tool changed.
    pub fn sync_eraser_tool(app: &mut crate::PhotocraftApp) -> bool {
        use crate::state::Tool;
        if app.drag.is_some() {
            return false;
        }
        match app.stylus.take_end_flip() {
            Some(true) if app.ui.tool != Tool::Eraser => {
                app.stylus.tool_before_eraser = Some(app.ui.tool);
                app.ui.tool = Tool::Eraser;
                true
            }
            Some(false) => match app.stylus.tool_before_eraser.take() {
                Some(t) if app.ui.tool == Tool::Eraser => {
                    app.ui.tool = t;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Pressure for the next tool event (1 for a mouse).
    pub fn pressure(&self) -> f32 {
        self.sample().map_or(1.0, |s| s.pressure)
    }

    /// Start recording a drag's tilt/rotation.
    pub(crate) fn begin_stroke(&mut self) {
        // A new stroke's first moves interpolate from this frame's samples, never from a sample
        // left over from an earlier stroke.
        self.prev = None;
        self.stroke.clear();
        self.times.clear();
        self.record_point();
    }

    /// Record the tilt/rotation and time of a point just added to the drag.
    pub(crate) fn record_point(&mut self) {
        let s = self.sample().unwrap_or_default();
        self.stroke.push([s.tilt_x, s.tilt_y, s.rotation]);
        self.times.push(self.clock_ms);
    }

    /// Time of the drag's last recorded point.
    pub(crate) fn last_point_ms(&self) -> Option<f64> {
        self.times.last().copied()
    }

    /// Time of drag point `i` (the last known time past the end).
    pub(crate) fn point_ms(&self, i: usize) -> f64 {
        self.times.get(i).or(self.times.last()).copied().unwrap_or(0.0)
    }

    /// Stroke points for `paint.stroke`: `[x, y, pressure]`, extended with
    /// `tiltX, tiltY, rotation` when the drag carried any tilt or rotation, and with `timeMs`
    /// (from the drag's first point) when its points carry times.
    pub fn stroke_points(&self, points: &[[f64; 3]]) -> Vec<Vec<f64>> {
        let has_pose = self.stroke.iter().any(|t| t.iter().any(|v| *v != 0.0));
        let t0 = self.point_ms(0);
        let has_time = self.times.iter().any(|t| *t != t0);
        points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut v = vec![p[0], p[1], p[2]];
                if has_pose || has_time {
                    let t = self.stroke.get(i).or(self.stroke.last()).copied().unwrap_or_default();
                    v.extend(t.map(f64::from));
                }
                if has_time {
                    v.push(self.point_ms(i) - t0);
                }
                v
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(phase: egui::TouchPhase, force: Option<f32>) -> egui::Event {
        egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(0), phase, pos: egui::pos2(1.0, 1.0), force }
    }

    fn pen(pressure: f32) -> PenSample {
        PenSample { pressure, ..Default::default() }
    }

    #[test]
    fn the_feed_queues_every_sample_in_order_and_keeps_the_latest() {
        let feed = StylusFeed::default();
        for p in [0.1, 0.2, 0.3] {
            feed.set(Some(pen(p)));
        }
        assert_eq!(feed.get().map(|s| s.pressure), Some(0.3));
        assert_eq!(feed.drain().iter().map(|s| s.pressure).collect::<Vec<_>>(), vec![0.1, 0.2, 0.3]);
        assert!(feed.drain().is_empty(), "drained once");
        feed.set(None);
        assert_eq!(feed.get(), None, "a lifted pen is not queued");
        assert!(feed.drain().is_empty());
        // A UI that stops draining keeps only the newest samples.
        for i in 0..FEED_QUEUE + 10 {
            feed.set(Some(pen((i % 100) as f32 / 100.0)));
        }
        assert_eq!(feed.drain().len(), FEED_QUEUE);
    }

    #[test]
    fn each_move_takes_its_share_of_the_frames_pen_samples() {
        let mut s = Stylus::default();
        s.feed.set(Some(pen(0.2)));
        s.update(&[]);
        s.begin_stroke();
        // Next frame: the pen reported 0.4, 0.6 and 0.8 while six pointer moves arrived.
        for p in [0.4, 0.6, 0.8] {
            s.feed.set(Some(pen(p)));
        }
        s.update(&[]);
        let got: Vec<f32> = (0..6)
            .map(|k| {
                s.select(k, 6);
                s.pressure()
            })
            .collect();
        let want = [0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-6), "{got:?}");
        s.clear_selection();
        assert_eq!(s.pressure(), 0.8, "back to the current sample");
    }

    #[test]
    fn a_second_update_in_the_same_frame_keeps_its_samples() {
        // The canvas runs once per view, and egui may run a frame in several passes.
        let mut s = Stylus::default();
        s.feed.set(Some(pen(0.4)));
        s.feed.set(Some(pen(0.8)));
        s.update_for_frame(7, &[]);
        s.update_for_frame(7, &[]);
        s.select(0, 2);
        assert!((s.pressure() - 0.4).abs() < 1e-6, "{}", s.pressure());
    }

    #[test]
    fn a_new_stroke_never_starts_from_an_old_strokes_pressure() {
        let mut s = Stylus::default();
        s.feed.set(Some(pen(0.9)));
        s.update(&[]);
        s.feed.set(Some(pen(0.1)));
        s.update(&[]);
        s.begin_stroke();
        s.select(0, 2);
        assert!((s.pressure() - 0.1).abs() < 1e-6, "{}", s.pressure());
    }

    #[test]
    fn a_mouse_or_use_pressure_off_paints_at_full_pressure() {
        let mut s = Stylus::default();
        s.update(&[egui::Event::PointerMoved(egui::pos2(1.0, 1.0))]);
        s.select(0, 1);
        assert_eq!(s.pressure(), 1.0);
        s.feed.set(Some(pen(0.3)));
        s.use_pressure = false;
        s.update(&[]);
        s.select(0, 1);
        assert_eq!(s.pressure(), 1.0);
    }

    #[test]
    fn rotation_between_samples_takes_the_short_way_round() {
        let mut s = Stylus::default();
        s.feed.set(Some(PenSample { rotation: 350.0, ..pen(1.0) }));
        s.update(&[]);
        s.feed.set(Some(PenSample { rotation: 10.0, ..pen(1.0) }));
        s.update(&[]);
        s.select(0, 2);
        let r = s.sample().map_or(-1.0, |p| p.rotation);
        assert!(r.abs() < 1e-3 || (r - 360.0).abs() < 1e-3, "halfway from 350° to 10° is 0°, got {r}");
    }

    #[test]
    fn mouse_is_full_pressure() {
        let mut s = Stylus::default();
        s.update(&[egui::Event::PointerMoved(egui::pos2(3.0, 4.0))]);
        assert_eq!(s.pressure(), 1.0);
        assert_eq!(s.sample(), None);
    }

    #[test]
    fn a_finger_paints_at_full_tip_size() {
        let mut s = Stylus::default();
        s.feed.report(PointerSource::Touch, None);
        s.update(&[touch(egui::TouchPhase::Start, Some(0.2))]);
        assert!(s.is_finger());
        assert_eq!(s.pressure(), 1.0, "fingers never use contact force as brush pressure");
        assert_eq!(s.sample(), None);
    }

    #[test]
    fn touch_force_drives_pressure_until_lift() {
        let mut s = Stylus::default();
        s.update(&[touch(egui::TouchPhase::Start, Some(0.25))]);
        assert_eq!(s.pressure(), 0.25);
        s.update(&[touch(egui::TouchPhase::Move, Some(0.8)), touch(egui::TouchPhase::Move, Some(1.7))]);
        assert_eq!(s.pressure(), 1.0, "clamped");
        s.update(&[touch(egui::TouchPhase::Move, Some(f32::NAN))]);
        assert_eq!(s.pressure(), 1.0);
        s.update(&[touch(egui::TouchPhase::Move, Some(0.4))]);
        assert_eq!(s.pressure(), 0.4);
        // The lift frame still paints at the last force; the next frame is back to the mouse.
        s.update(&[touch(egui::TouchPhase::End, None)]);
        assert_eq!(s.pressure(), 0.4);
        s.update(&[]);
        assert_eq!(s.pressure(), 1.0);
    }

    #[test]
    fn feed_wins_and_is_sanitized() {
        let mut s = Stylus::default();
        s.update(&[touch(egui::TouchPhase::Start, Some(0.3))]);
        s.feed.set(Some(PenSample { pressure: 0.6, tilt_x: 120.0, tilt_y: -30.0, rotation: -90.0, eraser: false }));
        assert_eq!(s.sample(), Some(PenSample { pressure: 0.6, tilt_x: 90.0, tilt_y: -30.0, rotation: 270.0, eraser: false }));
        s.feed.set(None);
        assert_eq!(s.pressure(), 0.3);
    }

    #[test]
    fn pen_samples_reach_the_brush_stroke() {
        use crate::canvas::{ToolEvent, tool_event};
        use serde_json::json;
        let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", json!({"width": 120, "height": 80})).unwrap();
        app.session.execute("layer.new.layer", json!({})).unwrap();
        app.ui.tool = crate::state::Tool::Brush;
        let m = egui::Modifiers::NONE;
        app.stylus.feed.set(Some(PenSample { pressure: 0.2, tilt_x: 40.0, tilt_y: 0.0, rotation: 30.0, eraser: false }));
        let pr = app.stylus.pressure();
        tool_event(&mut app, ToolEvent::Down { x: 10.0, y: 40.0, pressure: pr }, m);
        app.stylus.feed.set(Some(PenSample { pressure: 0.9, tilt_x: 10.0, tilt_y: -5.0, rotation: 60.0, eraser: false }));
        let pr = app.stylus.pressure();
        tool_event(&mut app, ToolEvent::Move { x: 100.0, y: 40.0, pressure: pr }, m);
        tool_event(&mut app, ToolEvent::Up { x: 100.0, y: 40.0 }, m);
        let (id, p) = app.session.journal.iter().rev().find(|(id, _)| id == "paint.stroke").cloned().unwrap();
        assert_eq!(id, "paint.stroke");
        assert_eq!(p["points"][0], json!([10.0, 40.0, 0.2f32 as f64, 40.0, 0.0, 30.0]));
        assert_eq!(p["points"][1], json!([100.0, 40.0, 0.9f32 as f64, 10.0, -5.0, 60.0]));
        // A mouse stroke stays at full pressure with plain [x, y, p] points.
        app.stylus.feed.set(None);
        let pr = app.stylus.pressure();
        tool_event(&mut app, ToolEvent::Down { x: 10.0, y: 60.0, pressure: pr }, m);
        tool_event(&mut app, ToolEvent::Up { x: 90.0, y: 60.0 }, m);
        let (_, p) = app.session.journal.iter().rev().find(|(id, _)| id == "paint.stroke").cloned().unwrap();
        assert_eq!(p["points"][0], json!([10.0, 60.0, 1.0]));
    }

    #[test]
    fn stroke_points_carry_tilt_only_when_present() {
        let mut s = Stylus::default();
        let pts = [[0.0, 0.0, 0.5], [10.0, 0.0, 0.7], [20.0, 0.0, 0.7]];
        s.begin_stroke();
        s.record_point();
        assert_eq!(s.stroke_points(&pts)[1], vec![10.0, 0.0, 0.7]);
        s.feed.set(Some(PenSample { pressure: 0.7, tilt_x: 30.0, tilt_y: 10.0, rotation: 45.0, eraser: false }));
        s.record_point();
        let out = s.stroke_points(&pts);
        assert_eq!(out[0], vec![0.0, 0.0, 0.5, 0.0, 0.0, 0.0]);
        assert_eq!(out[2], vec![20.0, 0.0, 0.7, 30.0, 10.0, 45.0]);
    }

    #[test]
    fn use_pressure_off_ignores_pen_and_touch() {
        let mut s = Stylus { use_pressure: false, ..Default::default() };
        s.update(&[touch(egui::TouchPhase::Start, Some(0.3))]);
        s.feed.set(Some(PenSample { pressure: 0.2, tilt_x: 40.0, ..Default::default() }));
        assert_eq!((s.sample(), s.pressure()), (None, 1.0));
        s.begin_stroke();
        assert_eq!(s.stroke_points(&[[1.0, 2.0, 1.0]]), vec![vec![1.0, 2.0, 1.0]], "no tilt either");
    }

    #[test]
    fn eraser_end_switches_tools_once_and_never_mid_drag() {
        use crate::canvas::{ToolEvent, tool_event};
        use crate::state::Tool;
        let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", serde_json::json!({"width": 60, "height": 60})).unwrap();
        app.ui.tool = Tool::Brush;
        let pen = |eraser| Some(PenSample { pressure: 0.5, eraser, ..Default::default() });
        // A mouse never switches.
        assert!(!Stylus::sync_eraser_tool(&mut app));
        app.stylus.feed.set(pen(true));
        assert!(Stylus::sync_eraser_tool(&mut app));
        assert_eq!(app.ui.tool, Tool::Eraser);
        assert!(!Stylus::sync_eraser_tool(&mut app), "reported once");
        // Flipping back during a drag waits for the drag to end.
        tool_event(&mut app, ToolEvent::Down { x: 10.0, y: 10.0, pressure: 0.5 }, egui::Modifiers::NONE);
        app.stylus.feed.set(pen(false));
        assert!(!Stylus::sync_eraser_tool(&mut app));
        assert_eq!(app.ui.tool, Tool::Eraser);
        tool_event(&mut app, ToolEvent::Up { x: 20.0, y: 10.0 }, egui::Modifiers::NONE);
        assert!(Stylus::sync_eraser_tool(&mut app));
        assert_eq!(app.ui.tool, Tool::Brush, "the tip restores the previous tool");
        // The eraser end while the Eraser is already chosen remembers nothing to restore.
        app.ui.tool = Tool::Eraser;
        app.stylus.feed.set(pen(true));
        assert!(!Stylus::sync_eraser_tool(&mut app));
        app.stylus.feed.set(pen(false));
        assert!(!Stylus::sync_eraser_tool(&mut app));
        assert_eq!(app.ui.tool, Tool::Eraser);
    }

    #[test]
    fn connection_starts_off_and_only_the_platform_sets_it() {
        let feed = StylusFeed::default();
        assert!(!feed.connected());
        assert!(!feed.ever_connected());
        feed.report(PointerSource::Pen, Some(PenSample::default()));
        assert!(!feed.ever_connected(), "contact alone is not a connection report");
        assert!(feed.set_connected(true));
        assert!(feed.connected());
        assert!(feed.ever_connected());
        assert!(!feed.set_connected(true), "unchanged");
        assert!(feed.set_connected(false));
        assert!(!feed.connected());
        assert!(feed.ever_connected(), "disconnect keeps settings available");
    }

    #[test]
    fn barrel_double_tap_and_long_press_are_distinct() {
        let mut w = BarrelWatch::default();
        assert_eq!(w.update(true, 0.0), None);
        assert_eq!(w.update(false, 40.0), None, "one short press waits for a second");
        assert_eq!(w.update(true, 120.0), None);
        assert_eq!(w.update(false, 160.0), Some(PenGesture::DoubleTap));

        let mut held = BarrelWatch::default();
        assert_eq!(held.update(true, 0.0), None);
        assert_eq!(held.update(true, BarrelWatch::LONG_MS), Some(PenGesture::LongPress));
        assert_eq!(held.update(false, BarrelWatch::LONG_MS + 20.0), None, "releasing a long-press is not a tap");
        assert_eq!(held.update(true, 1000.0), None);
        assert_eq!(held.update(false, 1040.0), None, "the long-press consumed the previous tap");
    }

    #[test]
    fn host_and_barrel_gestures_do_not_double_fire() {
        let feed = StylusFeed::default();
        feed.push_gesture(PenGesture::DoubleTap, 1_000.0);
        assert!(feed.report_barrel(true, 1_050.0).is_none());
        assert!(feed.report_barrel(false, 1_080.0).is_none());
        assert!(feed.report_barrel(true, 1_140.0).is_none());
        assert_eq!(feed.report_barrel(false, 1_180.0), Some(PenGesture::DoubleTap));
        assert_eq!(feed.take_gestures(), vec![PenGesture::DoubleTap], "the barrel echo of the host action is dropped");
        feed.push_gesture(PenGesture::LongPress, 2_000.0);
        feed.push_gesture(PenGesture::LongPress, 2_100.0);
        assert_eq!(feed.take_gestures(), vec![PenGesture::LongPress]);
    }

    #[test]
    fn double_tap_swaps_the_current_tool_and_the_eraser_then_back() {
        use crate::state::Tool;
        let mut stylus = Stylus::default();
        let mut tool = Tool::Brush;
        stylus.note_tool(tool);
        stylus.toggle_eraser_tool(&mut tool);
        assert_eq!(tool, Tool::Eraser);
        stylus.toggle_eraser_tool(&mut tool);
        assert_eq!(tool, Tool::Brush);
        stylus.toggle_eraser_tool(&mut tool);
        assert_eq!(tool, Tool::Eraser);
    }

    #[test]
    fn double_tap_previous_swaps_back_and_forth() {
        use crate::state::Tool;
        let mut stylus = Stylus::default();
        let mut tool = Tool::Brush;
        stylus.note_tool(tool);
        stylus.switch_previous_tool(&mut tool);
        assert_eq!(tool, Tool::Brush, "nothing to return to yet");
        tool = Tool::Pencil;
        stylus.note_tool(tool);
        stylus.switch_previous_tool(&mut tool);
        assert_eq!(tool, Tool::Brush);
        stylus.switch_previous_tool(&mut tool);
        assert_eq!(tool, Tool::Pencil);
    }
}
