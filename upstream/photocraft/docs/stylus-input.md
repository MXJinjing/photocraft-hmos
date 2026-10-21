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
| `barrelButton` | `pressed`, `timestampMs` | Side-button state; the shared recognizer derives double-tap or the held-button squeeze fallback. |
| `gesture` | `gesture`, `timestampMs` | An adapter has recognized `doubleTap` or `squeeze`. The legacy `longPress` name is accepted as an alias for `squeeze`. Preferences decide the action. |
| `connection` | `connected`, optional `deviceName` | Whether a pen is attached, independently of screen contact, with its current InputKit name when available. |

A sample contains `pressure` (0–1), `tiltX` and `tiltY` (degrees, −90–90), `rotation`
(degrees, normalized to 0–360 exclusive), and `eraser` (boolean). Omitted sample fields default
to pressure 1, zero tilt/rotation and eraser false. The shared sink clamps numeric ranges and
replaces non-finite sample values.

Connection metadata is a snapshot. Disconnect clears the previous device's name. Names are
limited to 128 non-control characters. Version 1 packets without metadata continue to work.
Battery state belongs to the operating system and is neither queried nor displayed by PhotoCraft.
The title-bar pen icon is always available,
including before any connection, and its menu shows the current device status above preferences.

All producers sharing a feed must use the same timestamp clock. Web/native-container adapters
use Unix epoch milliseconds, matching `Date.now()`. Convert SDK timestamps before submission;
SDK uptime and epoch timestamps must not be mixed. Non-finite action times are ignored.
Repeated identical gestures within 350 ms are suppressed across native actions and browser
side-button recognition. The queue holds at most eight actions. The shared side-button
recognizer uses 400 ms for double-tap and 450 ms for its held-side-button squeeze fallback.

## Help › Stylus Debug

Open **Help › Stylus Debug…** without needing a document. On HarmonyOS this uses the existing
native egui immediate-viewport sub-window host, with its own system title bar, surface and input;
web hosts without native viewports embed it instead. On HarmonyOS use a real Pad and a pen
that supports tilt and the desired body gestures; an emulator can check the window layout and
transport but cannot prove physical sensor or gesture support.

- Touch the test area, vary pressure, tilt left/right/up/down, then lift. The window retains the
  last sanitized device sample (before pressure curves and even when tablet pressure is off).
  The pressure-width test marks are session-only and never modify the document.
- Tilt X/Y are degrees. The ring arrow is the pen axis projected onto the screen: 0° up,
  90° right, 180° down and 270° left. Its length increases with inclination from vertical.
  Upright has no bearing. This is not barrel rotation or the physical nib's chisel orientation;
  the current HarmonyOS contact bridge does not report barrel rotation. The HarmonyOS host sends
  the display's quarter-turn rotation and portrait/landscape state to the shared feed, which
  converts the natural panel tilt axes into the current screen axes before painting or display.
  Landscape panels add the half turn used by the Pad pen driver. Zero tilt alone cannot distinguish
  an upright pen from hardware that does not report tilt.
- Proximity is recorded separately from contact. The single contact-status line reads **Hovering**
  or **Pen contact** and includes the latest X/Y point, without replacing the last contact tilt.
  The sample counter also shows the pen sampling rate over the most recent second.
- Double-tap and squeeze each have an accepted-event count and a 1.2-second indicator. Events
  within the existing 350 ms deduplication window count once. No event is not proof of missing
  hardware support. Squeeze is now named consistently in menus and preferences; the existing
  `tools.stylusLongPress` preference key is retained for saved-settings compatibility.
- **Test gestures only** defaults on while the window is open; gesture shortcuts are consumed
  without changing tools or opening canvas menus. Closing the window restores normal behavior.
  Uncheck it to test configured shortcut actions. **Reset test** resets this view, not input
  queues or settings.

Automated checks cover screen-axis geometry, sanitized readings, non-consuming observation,
legacy input compatibility, deduplication, gesture isolation and UI reset behavior. Physical
Pad tests must still verify that moving the actual pen changes the readings and both gestures
reach their indicators.

## Native web-container transport, version 1

Dispatch `photocraft-stylus-input` on `window` with an **object** in `CustomEvent.detail`:

```javascript
window.dispatchEvent(new CustomEvent('photocraft-stylus-input', {
  detail: {
    version: 1,
    type: 'gesture',
    gesture: 'squeeze',
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
`HarmonyStylusAdapter.ets` uses Pen Kit and InputKit:

- SDK `doubleTap` becomes the shared `doubleTap` action.
- SDK `squeeze` becomes the shared `squeeze` action (轻捏).
- Device changes and a one-second poll publish connection state and the InputKit device name.
  Device enumeration is serialized; triggers received during a scan coalesce into one immediate
  follow-up, so a scan slower than the polling interval is still accepted without overlapping calls.
- No Bluetooth or NearLink permission is requested. The adapter does not query or display battery state.
- Registration failures are handled separately, allowing supported capabilities to keep working.
- Start/stop are idempotent; asynchronous connection queries from a previous lifecycle are ignored.

`Index.ets` owns the adapter and forwards packets to the native Rust host. It starts/stops
the adapter with the page lifecycle. The Rust input bridge submits the shared contract.

Android and iPadOS native adapters are not implemented in this repository. They can implement
the same lifecycle and packet contract using their own SDKs without changing the canvas,
context-menu behavior or preferences. Standard browser pen events continue to use the existing
Pointer Events adapter wherever the browser supplies them. Available hardware gestures and
reported sample fields remain dependent on the platform/device.

## Verification

Rust tests cover decoding, validation, normalized actions and cross-source deduplication.
The fixture `crates/ui-egui/tests/fixtures/stylus-packets.json` is shared with the ArkTS adapter
test, which generates packets from the actual factories and checks lifecycle, transport,
startup state, action mapping, serialized slow scans and partial SDK availability:

```sh
node scripts/test_stylus_adapter.cjs /path/to/typescript
```

Run this command from the repository root, passing the TypeScript compiler directory provided
by DevEco Studio's Hvigor installation. It mocks SDKs; real pen behavior still requires device
testing. Rust/WASM and HAP builds validate the actual consumers and ArkTS implementation.
