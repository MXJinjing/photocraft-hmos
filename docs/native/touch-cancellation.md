# Touch cancellation without a synthetic stroke endpoint

HarmonyOS XComponent non-pen `CANCEL` is forwarded as `{"kind":"blur"}` while no pen is in contact. That packet has no pointer position. Previously, its default `(0, 0)` coordinates became a normal egui release, and the brush appended a straight segment toward the surface origin. Canvas clipping hid the segment outside the document.

The native input adapter now remembers the last pointer position it actually emitted, in logical points. Cancellation takes that position before clearing the contacts, releases any held button there, and hides the pointer. It preserves the existing stroke and ignores cancellation packet coordinates. Rejected mouse echoes, suppressed palms and contact-time pen hover reports do not update the position. Main and child windows use the same cancellation method without a caller-supplied coordinate.

This fixes the cancellation path. Determining whether a physical palm triggers an OS CANCEL on a particular Pad still needs device logging.

## Automated verification

Native adapter unit tests cover touch/mouse/pen cancellation, omitted/zero/unrelated/non-finite cancellation coordinates, scaling, ignored echoes, repeated cancellation, navigation cleanup, egui's release-frame interaction position and a new stroke after cancellation. Existing tests cover two-finger promotion, pen takeover and cursor behavior.

Run `cargo test --offline --manifest-path native/rust/Cargo.toml`.

The emulator test runs the real native worker and rendering, entering at `native.input`. It does not generate an OS palm event or exercise the C++ XComponent callback. Use a fixed 2880 × 1920 Pad emulator and a separate application ending in `.palmtest`; do not replace a user's working application.

Build a local isolated copy with the updated ARM64 library, main HAP and `entry@ohosTest` HAP. Change the bundle name and all custom UTD/file-open namespaces together in that copy. Use unsigned HAPs on the emulator. Keep those test identity changes out of public source. Install both HAPs, then run:

```sh
python3 scripts/test_touch_cancel_simulator.py \
  --bundle io.github.storytold.photocraft.hmos.palmtest \
  --output-dir work/palm-cancel-repro/after
```

The runner enables finger painting and draws the same short stroke in two new 1024 × 768 documents. One ends with an empty touch snapshot, the other with `blur`. Each then receives a second stroke. The verifier checks fixed-size screenshots for equivalent committed ink, preservation of the original stroke and successful next strokes; native document revisions must increase for the second stroke.

To verify saved reproduction evidence, use `--verify-existing --phase before --output-dir <baseline-directory>`. The original baseline needs six images (`normal`/`cancel` × `before`/`held`/`after`); the fixed replay includes two additional `resumed` images. Baseline verification must detect the extra cancelled-stroke pixels. Fixed verification must find no more than 20 extra or missing pixels relative to normal lift.

## Verification result (2026-10-23)

| Replay | Normal stroke ink pixels | Cancelled stroke ink pixels | Extra / missing cancelled pixels |
| --- | ---: | ---: | ---: |
| Before | 1,210 | 12,311 | 11,101 / 0 |
| After | 1,210 | 1,210 | 0 / 0 |

Both next strokes added 1,054 ink pixels, with document revisions `[2, 3, 2, 3]`. All 105 native unit tests and the native C++ input bridge test passed. ARM64 release Rust, the public HAP, and independent emulator HAPs built successfully; both HAPs passed ABI, asset and bundle/file-association verification. Fixed and baseline screenshots/logs are saved under the ignored `work/palm-cancel-repro/` directory.

The production-library Clippy check passed with `-D warnings -A dead_code` (host builds do not use the OHOS worker). The full strict all-targets run remains blocked by existing dead-code/test lint errors. Full offline upstream metadata is unavailable because `aws-lc-rs v1.18.2` is not cached; offline metadata without dependency expansion passed. No upstream engine files were changed.
