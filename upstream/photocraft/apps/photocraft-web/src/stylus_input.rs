//! Browser adapter for standard Pointer Events and the platform-independent host protocol.

use egui::Context;
use photocraft_ui_egui::stylus::{StylusFeed, StylusInput};
use photocraft_ui_egui::stylus_protocol::{HOST_EVENT, STATE_PROPERTY, StylusPacket};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::closure::Closure;

pub fn install(canvas: &web_sys::HtmlCanvasElement, feed: StylusFeed, ctx: Context) {
    listen_pointer_events(canvas, feed.clone(), ctx.clone());
    listen_host(feed, ctx);
}

// eframe does not forward pen-specific pressure/tilt, so feed them before canvas tool handling.
// Keep the last sample through pointerup for the stroke's final point.
fn listen_pointer_events(target: &web_sys::HtmlCanvasElement, feed: photocraft_ui_egui::stylus::StylusFeed, ctx: egui::Context) {
    use photocraft_ui_egui::stylus::{PenSample, StylusInput};
    use wasm_bindgen::closure::Closure;
    for kind in ["pointerdown", "pointermove", "pointerup", "pointercancel", "pointerleave"] {
        let feed = feed.clone();
        let ctx = ctx.clone();
        let cb = Closure::<dyn FnMut(web_sys::PointerEvent)>::new(move |e: web_sys::PointerEvent| {
            let ty = e.type_();
            let source = photocraft_ui_egui::stylus::PointerSource::from_pointer_type(&e.pointer_type());
            let leaving = ty == "pointercancel" || ty == "pointerleave";
            // W3C Pointer Events: bit 1 (2) is the barrel button, bit 5 (32) the eraser.
            let barrel = source == photocraft_ui_egui::stylus::PointerSource::Pen && !leaving && e.buttons() & 2 != 0;
            if feed.submit(StylusInput::BarrelButton { pressed: barrel, timestamp_ms: js_sys::Date::now() }) {
                ctx.request_repaint();
            }
            // Keep the last pen sample through `pointerup` so the stroke's last points keep their
            // pressure; the next non-pen event switches the source.
            if ty == "pointerup" && source == photocraft_ui_egui::stylus::PointerSource::Pen {
                return;
            }
            let down = e.buttons() != 0 && !leaving;
            let eraser = e.buttons() & 32 != 0;
            let sample = (source == photocraft_ui_egui::stylus::PointerSource::Pen && down).then(|| PenSample {
                pressure: e.pressure(),
                tilt_x: e.tilt_x() as f32,
                tilt_y: e.tilt_y() as f32,
                rotation: e.twist() as f32,
                eraser,
            });
            feed.submit(StylusInput::Pointer { source, sample });
        });
        if target.add_event_listener_with_callback(kind, cb.as_ref().unchecked_ref()).is_ok() {
            cb.forget();
        }
    }
}

/// Any native container can dispatch this event; the consumer has no SDK-specific branches.
fn listen_host(feed: StylusFeed, ctx: Context) {
    let Some(window) = web_sys::window() else { return };
    let bootstrap_feed = feed.clone();
    let bootstrap_ctx = ctx.clone();
    let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Ok(detail) = js_sys::Reflect::get(&event, &"detail".into()) else { return };
        submit_host_detail(&feed, &ctx, &detail, false);
    });
    if window.add_event_listener_with_callback(HOST_EVENT, callback.as_ref().unchecked_ref()).is_ok() {
        callback.forget();
    }
    // A connection update can precede WASM startup. Replay the cached state once; the host
    // stores only connection packets here, never actions that could unexpectedly open a menu.
    if let Ok(detail) = js_sys::Reflect::get(&window, &STATE_PROPERTY.into()) {
        submit_host_detail(&bootstrap_feed, &bootstrap_ctx, &detail, true);
    }
}

fn submit_host_detail(feed: &StylusFeed, ctx: &Context, detail: &wasm_bindgen::JsValue, snapshot: bool) {
    // Host detail is a JSON object. Serialising once gives the Rust decoder one validated
    // path for all adapters, including unknown versions and malformed packets.
    let Ok(json) = js_sys::JSON::stringify(detail) else { return };
    let Some(json) = json.as_string() else { return };
    let Ok(packet) = StylusPacket::from_json(&json) else { return };
    if snapshot && !matches!(packet.input, StylusInput::Connection { .. }) {
        return;
    }
    if feed.submit(packet.input) {
        ctx.request_repaint();
    }
}
