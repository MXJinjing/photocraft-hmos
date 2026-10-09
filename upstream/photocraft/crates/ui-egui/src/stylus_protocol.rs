//! Versioned transport for native containers. The shared stylus sink does not depend on a
//! platform SDK; browser/native adapters submit the same [`StylusInput`] values.

use serde::{Deserialize, Serialize};

use crate::stylus::StylusInput;

pub const HOST_EVENT: &str = "photocraft-stylus-input";
pub const VERSION: u32 = 1;
pub const STATE_PROPERTY: &str = "__photocraftStylusState";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StylusPacket {
    pub version: u32,
    #[serde(flatten)]
    pub input: StylusInput,
}

impl StylusPacket {
    pub fn new(input: StylusInput) -> Self {
        Self { version: VERSION, input }
    }

    /// Ignore unsupported versions rather than guessing an action. Limit host packet size
    /// before decoding; a stylus packet never carries documents or bulk data.
    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > 4096 {
            return Err("stylus packet exceeds 4096 bytes".into());
        }
        let packet: Self = serde_json::from_str(json).map_err(|e| format!("invalid stylus packet: {e}"))?;
        if packet.version != VERSION {
            return Err(format!("unsupported stylus protocol version {}", packet.version));
        }
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stylus::{PenGesture, PenSample, PointerSource, StylusFeed};

    #[test]
    fn arkts_factory_packets_follow_the_rust_contract() {
        // The ArkTS adapter test generates these packets from its actual factory classes and
        // compares against the same fixture, catching producer/consumer spelling or unit drift.
        let packets: Vec<serde_json::Value> = serde_json::from_str(include_str!("../tests/fixtures/stylus-packets.json")).unwrap();
        let feed = StylusFeed::default();
        for value in packets {
            feed.submit(StylusPacket::from_json(&value.to_string()).unwrap().input);
        }
        assert!(feed.connected());
        assert_eq!(feed.source(), PointerSource::Touch);
        assert!(feed.get().is_none());
        assert_eq!(feed.take_gestures(), [PenGesture::DoubleTap, PenGesture::LongPress]);
    }

    #[test]
    fn independent_host_packets_use_the_same_sink_and_units() {
        let feed = StylusFeed::default();
        let packets = [
            r#"{"version":1,"type":"connection","connected":true}"#,
            r#"{"version":1,"type":"pointer","source":"pen","sample":{"pressure":0.25,"tiltX":30,"tiltY":-20,"rotation":90,"eraser":false}}"#,
            r#"{"version":1,"type":"gesture","gesture":"doubleTap","timestampMs":1000}"#,
            r#"{"version":1,"type":"gesture","gesture":"longPress","timestampMs":2000}"#,
        ];
        for json in packets {
            assert!(feed.submit(StylusPacket::from_json(json).unwrap().input));
        }
        assert!(feed.connected());
        assert_eq!(feed.source(), PointerSource::Pen);
        assert_eq!(feed.get(), Some(PenSample { pressure: 0.25, tilt_x: 30.0, tilt_y: -20.0, rotation: 90.0, eraser: false }));
        assert_eq!(feed.take_gestures(), [PenGesture::DoubleTap, PenGesture::LongPress]);
        let json = r#"{"version":1,"type":"pointer","source":"touch","sample":null}"#;
        feed.submit(StylusPacket::from_json(json).unwrap().input);
        assert_eq!(feed.source(), PointerSource::Touch);
        assert_eq!(feed.get(), None);
    }

    #[test]
    fn unsupported_or_malformed_packets_do_not_reach_the_sink() {
        let feed = StylusFeed::default();
        for json in [
            r#"{"version":2,"type":"connection","connected":true}"#,
            r#"{"type":"connection","connected":true}"#,
            r#"{"version":1,"type":"connection","connected":"true"}"#,
            r#"{"version":1,"type":"gesture","gesture":"squeeze","timestampMs":1000}"#,
            r#"{"version":1,"type":"gesture","gesture":"longPress"}"#,
            r#"{"version":1,"type":"pointer","source":"unknown","sample":null}"#,
            r#"{"version":1,"type":"gesture","gesture":"doubleTap","timestampMs":1e400}"#,
            r#"{"version":1,"type":"unknown"}"#,
        ] {
            assert!(StylusPacket::from_json(json).is_err(), "{json}");
        }
        assert!(StylusPacket::from_json(&" ".repeat(4097)).is_err());
        assert!(!feed.connected());
        assert!(feed.take_gestures().is_empty());
        assert!(!feed.submit(StylusInput::Gesture { gesture: PenGesture::DoubleTap, timestamp_ms: f64::NAN }));
        assert!(feed.take_gestures().is_empty());
    }

    #[test]
    fn standard_side_button_and_native_actions_are_deduplicated() {
        let feed = StylusFeed::default();
        feed.submit(StylusInput::Gesture { gesture: PenGesture::LongPress, timestamp_ms: 1000.0 });
        feed.submit(StylusInput::BarrelButton { pressed: true, timestamp_ms: 600.0 });
        feed.submit(StylusInput::BarrelButton { pressed: true, timestamp_ms: 1100.0 });
        assert_eq!(feed.take_gestures(), [PenGesture::LongPress]);
        feed.submit(StylusInput::BarrelButton { pressed: false, timestamp_ms: 1200.0 });
        feed.submit(StylusInput::Gesture { gesture: PenGesture::LongPress, timestamp_ms: 2000.0 });
        assert_eq!(feed.take_gestures(), [PenGesture::LongPress], "a later action still works");
    }
}
