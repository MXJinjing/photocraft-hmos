# Unified stylus input

PhotoCraft's canvas and tool preferences consume a platform-independent `StylusInput` through
`StylusFeed::submit` (`crates/ui-egui/src/stylus.rs`). Platform adapters translate their SDK's
events into this contract. They do not select tools or implement context menus themselves.

```text
Browser Pointer Events ──────────────────────┐
Native tablet monitors ──────────────────────┼─ StylusFeed ─ tool preferences ─ canvas / menus
Native host SDK ─ adapter ─ versioned packet ┘
```

The browser implementation is in `apps/photocraft-web/src/stylus_input.rs`. It reads pen
pressure, tilt, rotation, eraser and side buttons from standard Pointer Events. Native tablet
monitors can submit the same Rust values directly. A native web container uses the transport
below to provide additional pen actions and connection state.

## Contract

`StylusInput` has four variants. JSON field and enum names use camelCase.

| `type` | Fields | Meaning |
| --- | --- | --- |
| `pointer` | `source`, `sample` | Source is `mouse`, `pen` or `touch`. Sample is a pen reading or `null` when no pen is down. Non-pen samples are discarded. |
| `barrelButton` | `pressed`, `timestampMs` | Side-button state; the shared recognizer derives double-tap or long-press. |
| `gesture` | `gesture`, `timestampMs` | An adapter has recognized `doubleTap` or `longPress`. Preferences decide the action. |
| `connection` | `connected` | Whether a pen is attached, independently of contact with the screen. |

A sample contains `pressure` (0–1), `tiltX` and `tiltY` (degrees, −90–90), `rotation`
(degrees, normalized to 0–360 exclusive), and `eraser` (boolean). Omitted sample fields default
to pressure 1, zero tilt/rotation and eraser false. The shared sink clamps numeric ranges and
replaces non-finite sample values.

All producers sharing a feed must use the same timestamp clock. Web/native-container adapters
use Unix epoch milliseconds, matching `Date.now()`. Convert SDK timestamps before submission;
SDK uptime and epoch timestamps must not be mixed. Non-finite action times are ignored.
Repeated identical gestures within 350 ms are suppressed across native actions and browser
side-button recognition. The queue holds at most eight actions. The shared side-button
recognizer uses 400 ms for double-tap and 450 ms for long-press.

## Native web-container transport, version 1

Dispatch `photocraft-stylus-input` on `window` with an **object** in `CustomEvent.detail`:

```javascript
window.dispatchEvent(new CustomEvent('photocraft-stylus-input', {
  detail: {
    version: 1,
    type: 'gesture',
    gesture: 'longPress',
    timestampMs: Date.now()
  }
}));
```

Each packet is `{ version: 1, ...StylusInput }`. The Rust decoder in
`crates/ui-egui/src/stylus_protocol.rs` rejects missing/unknown versions, unknown action/type
values, invalid field types and packets larger than 4096 bytes. The old
`photocraft-stylus` and `photocraft-stylus-connected` events are not supported.

Connection updates can arrive before WASM starts. Store the latest connection packet in
`window.__photocraftStylusState` before dispatching it. The browser reads this property after
registering its listener and accepts only a `connection` packet as the startup snapshot.
Never cache/replay gestures: that could open a menu again after a page reload.

## HarmonyOS implementation

`entry/src/main/ets/stylus/StylusProtocol.ets` defines typed packet factories, the injection
script and a `StylusAdapter` lifecycle interface (`start`, `stop`, `publishState`).
`HarmonyStylusAdapter.ets` is the implementation using Pen Kit and InputKit:

- SDK `doubleTap` becomes the shared `doubleTap` action.
- SDK `squeeze` becomes the shared `longPress` action used by the current preferences.
- Device changes and a one-second poll publish connection state.
- Registration failures are handled separately, allowing supported capabilities to keep working.
- Start/stop are idempotent; asynchronous connection queries from a previous lifecycle are ignored.

`Index.ets` owns the adapter and forwards packets to ArkWeb. It starts/stops the adapter with
the page lifecycle and asks it to publish current state after the web page loads.

Android and iPadOS native adapters are not implemented in this repository. They can implement
the same lifecycle and packet contract using their own SDKs without changing the canvas,
context-menu behavior or preferences. Standard browser pen events continue to use the existing
Pointer Events adapter wherever the browser supplies them. Available hardware gestures and
reported sample fields remain dependent on the platform/device.

## Verification

Rust tests cover decoding, validation, normalized actions and cross-source deduplication.
The fixture `crates/ui-egui/tests/fixtures/stylus-packets.json` is shared with the ArkTS adapter
test, which generates packets from the actual factories and checks lifecycle, transport,
startup state, action mapping and partial SDK availability:

```sh
node scripts/test_stylus_adapter.cjs /path/to/typescript
```

Run this command from the repository root, passing the TypeScript compiler directory provided
by DevEco Studio's Hvigor installation. It mocks SDKs; real pen behavior still requires device
testing. Rust/WASM and HAP builds validate the actual consumers and ArkTS implementation.
