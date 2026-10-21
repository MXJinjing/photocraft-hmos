# Graceful native startup

The ArkUI startup cover stays opaque while the document directory, window colour
space, editor, fonts and GPU are initialized. The XComponent is created underneath
the cover as soon as `native.init` has started the worker, so the cover does not
delay creation of the native surface.

The worker emits `first_frame` only after `render.paint` returns `true` for the
root surface. This means a UI frame has been submitted for presentation, not that
the compositor has confirmed physical display. `ready` still means GPU setup is
complete and must not dismiss the cover. Skipped, lost or outdated frames cannot
trigger the reveal. Attaching a replacement surface resets the notification.

On `first_frame`, ArkUI fades the opaque cover out over 320 ms with EaseOut and
scales the central brand group from 1 to 0.98. The editor remains stationary.
The cover blocks pointer input until the fade finishes, then is removed and
editor focus is restored. A generation check prevents an old animation callback
from touching a recreated page. Errors remain visible above the cover.

Verification:

- Native unit tests live in `native/rust/tests/unit/startup.rs`: skipped frames,
  exactly one notification, and replacement surfaces.
- Build with `scripts/dev.sh build` to rebuild both the Rust producer and ArkUI
  consumer. Packaging ArkUI against an older static library is insufficient:
  that producer does not emit `first_frame`.
- On the Pad emulator, capture the same screen dimensions before and after;
  check cold startup and relaunch, the uninterrupted startup cover, fade into
  the editor, and focus after the cover is removed. Logs should contain `ready`,
  `first_frame`, then `PhotoCraft startup reveal complete` in that order.
- Recheck document open, Save/Save As and external browser links after reveal.

## Local checks (2026-10-21)

The release ARM64 Rust library and debug HAP built successfully via
`scripts/dev.sh build`; the unsigned HAP passed `verify_native.py`. Upstream
`cargo metadata --offline` passed. The full native adapter suite passed with
97 tests (including the three first-presentation tests), using
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test
--offline --manifest-path native/rust/Cargo.toml` to limit temporary disk use.

Pad runtime verification is pending: `PhotoCraftPadAPI20` is offline and its
official screenshot command times out. The connected `127.0.0.1:5557` instance
is the 2in1 simulator. Do not treat that connection as Pad verification. Restart
the Pad only after confirming there are no unsaved documents to preserve.
