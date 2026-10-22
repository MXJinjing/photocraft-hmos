# Free Transform: stationary finger drift

2026-10-22

## Cause and fix

A single finger has a short grace period before tool ownership, allowing a second
finger to promote the contact to canvas navigation. `touch_nav::update` compared
Move reports with the initial contact position. After ownership, stationary reports
at the displaced position could restart that timer and repeatedly produce a
deferred tool press. Ownership now stays latched until release.

The canvas's early brush/eraser press path also ran during Free Transform.
It tested `app.drag`, but transforms store their gesture in `TransformPreview`.
Replayed Down events therefore restarted the transform with the original press
position against the current, already changed quadrilateral. An inside drag could
accumulate translation until that original position became an outside hit, then
accumulate rotation even though subsequent coordinates were identical. The early
painting path now yields to the existing `transform_owns_pointer` guard.

Pen input bypasses the finger grace timer, explaining the input-source difference.
The reproduced sequence includes initial movement followed by identical stationary
reports and idle frames. Exact constant coordinates from the first Down were also
tested and stayed geometrically unchanged; physical contact jitter was not measured.

## Regression coverage

- `touch_nav::tests::a_held_contact_cannot_rearm_the_deferred_press`: one fallback
  press after grace, then no repeated press from stationary reports.
- `input::tests::native_free_transform_single_contact_stays_still`: actual native
  input adapter plus editor frames, 135 combinations of Brush/Eraser/Move,
  touch/pen/mouse, inside/corner/edge/pivot/outside, and zero/tiny/normal movement.
  Checks stationary and idle frames, camera stability, actual movement, release
  stability and one-step transform undo.
- `TransformTouchTest.ets` and `scripts/test_transform_touch_simulator.py`: native
  input replay through the HarmonyOS worker in an independent `.transformtest`
  bundle. Fixed 2880 × 1920 screenshots compare red fixture pixels; native command
  logs additionally compare transform geometry. Stationary long presses may open
  a context menu, so their geometry is checked without treating menu occlusion as
  image movement. Moved cases must actually change geometry and rendered pixels.

## Simulator comparison

The baseline used the local October 21 unsigned HAP's native library in the same
independent test package; the final package uses the October 22 ARM64 source build.
With the same moved-then-held finger sequence, baseline red pixels changed by 6,889
while the contact stayed fixed. Pen and mouse changed by zero. The repaired run
changed zero pixels for all three sources. All purely stationary cases retained
their original quadrilateral. Screenshots and logs are in ignored
`work/transform-touch/{before,after}/`.

This verifies native replay and rendering on the Pad emulator, not physical Pad
sensor/firmware delivery. The regular installed application was not replaced.

## Running the checks

```sh
cargo test --offline --manifest-path native/rust/Cargo.toml
cd upstream/photocraft
cargo test --offline -p photocraft-ui-egui --lib touch_nav::tests
```

Build an independent emulator package with the `.transformtest` bundle ID, build
and install its `entry` and `entry@ohosTest` HAPs, then run (Python needs Pillow):

```sh
python3 scripts/test_transform_touch_simulator.py --phase after \
  --output-dir work/transform-touch/after
```

Use `--phase before` only with a baseline library: it requires finger drift and
stable pen/mouse controls. `--verify-existing` rechecks saved evidence without
operating the emulator. The script requires an emulator and an independent bundle.

## Build and suite results

- Native Rust suite: 98 passed, including all 135 transform combinations.
- Touch navigation suite: 10 passed.
- Offline Cargo metadata, 29-crate layering check and `cargo xtask wasm`: passed.
- ARM64 `scripts/build_native.sh`: passed; HAP build and `verify_native.py`: passed.
- Final emulator replay: passed for touch, pen and mouse; baseline drift reproduced.
- Strict Clippy remains blocked in unchanged code: UI `file_open.rs:130`
  (`manual_inspect`); native host-only dead-code diagnostics and existing test
  `unwrap_used` diagnostics. No Clippy diagnostic points to the added test bodies.
  Full diagnostic logs are in `work/transform-touch/*clippy.log`.
- Full UI library suite with access to the GPU: 2,803 passed, 2 failed,
  12 ignored. Remaining failures are
  `brush_panel::tests::every_section_draws_without_issuing_commands` (tab 0,
  section 1 unexpectedly emits one command) and
  `prefs_ui::tests::preference_labels_are_translated` (four Arkui theme names lack
  Japanese translations). The sandbox run additionally failed three GPU tests
  with `No adapter found`; all three pass with GPU access. These remaining UI
  failures and strict Clippy errors are outside the touch-transform changes and
  have not been altered in this fix. Logs: `work/transform-touch/ui-tests-gpu.log`.
