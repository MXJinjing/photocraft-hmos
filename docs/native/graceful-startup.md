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

## Pad verification (2026-10-21)

After the user confirmed there were no unsaved documents, the unresponsive
`PhotoCraftPadAPI20` process was stopped and cold-booted. Installation and tests
used `127.0.0.1:5555`, confirmed by `const.product.devicetype = tablet`, with
2880 x 1920 screenshots. The 2in1 instance at `127.0.0.1:5557` was untouched.

- First launch: `ready` at 18:34:46.006, `first_frame` at 18:34:46.136,
  reveal complete at 18:34:46.466 (330 ms after the notification reached ArkUI).
- Process relaunch: `ready` at 18:35:28.839, `first_frame` at 18:35:29.058,
  reveal complete at 18:35:29.392 (334 ms).
- Twenty sequential full-size screenshots captured system launch, the opaque
  branded loading cover, the reveal and the stable editor. No blank black frame
  appeared in these samples. Screenshot capture is not a frame-perfect recording
  and cannot rule out a shorter transient between samples.
- Both touching New Document and pressing Ctrl+N opened the new-document dialog
  after reveal. Escape closed it. This confirms the cover no longer blocks input
  and hardware-keyboard focus is restored. No documents were created or modified.
- Save/Save As and external-browser flows were not manually retested in this run.

Local evidence is in `work/graceful-startup/`: `pad-hilog.log`,
`pad-relaunch-hilog.log`, `frames.json`, `frame-00.png` through `frame-19.png`,
`startup-contact-sheet.jpg`, `after.png` and `keyboard.png`. The work directory
is ignored and is not part of the commit. Device log timestamps above use the
device's raw log time.
