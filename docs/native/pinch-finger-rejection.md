# Finger rejection after pinch

PR #23 guarded leftover touch snapshots, but did not guard the separate mouse
callback path. It also left axis callbacks unconditionally reporting Mouse to the
shared stylus feed. If an axis callback is last in an input batch, the canvas can
lose the finger classification used by `tools.blockFingerInput`.

`input::State` now retains finger ownership while contacts, navigation, palm
rejection, or the existing 300 ms post-navigation guard are active. Axis callbacks
preserve that owner; mouse callbacks are ignored before they can change the feed,
cancel the gesture, move a tool, or press Primary. Pen events retain their existing
priority. A genuine mouse press is accepted after the guard expires.

## Regression checks

- With the source from `origin/main` before this fix, the new
  `axis_does_not_reclassify_a_finger_as_mouse` and
  `mouse_echo_after_pinch_cannot_press_or_move_a_tool` tests fail.
- The fixed native unit suite passes all 55 tests, including a real editor test
  with finger blocking enabled and interleaved axis/touch/mouse events.
- The C++ Touch/Mouse/Axis/Hover callback test passes.
- ARM64 OHOS release library, default HAP, test HAP, and native package verification
  pass. Host clippy completes with existing dead-code and collapsible-if warnings.

## Simulator test

Verified on 2026-10-12 with HarmonyOS 6.0.0.48 / API 20, ARM64
`PhotoCraftPadAPI20`, 2880 × 1920, using the independent
`io.github.storytold.photocraft.pinchtest` bundle.

The committed `OpenHarmonyTestRunner` uses UiTest to execute 20 alternating
pinch-in/pinch-out gestures and a single-finger swipe after each gesture. It then
replays 20 staggered-release touch/axis/mouse callback sequences through the actual native worker,
alternating which finger lifts first. The remaining finger stays down for 700 ms
(longer than the post-lift guard), then moves before its final lift. Half of the
sequences insert a transient empty snapshot before re-reporting the remnant.
Each sequence also includes post-lift mouse press/move/release echoes. UiTest cannot
force a physical tablet's firmware to emit that exact echo sequence; callback
replay covers it explicitly. Finally, UiTest injects a pen stroke as a positive
control.

The companion `scripts/test_pinch_simulator.py` reads `document.inspect`
notifications from the process log via hdc and asserts
that the first three document revisions match and the pen stroke changes the
revision: **1 → 1 → 1 → 2**. Only the pen adds a `Brush Tool` history entry.
Screenshots show an unchanged white document with a changed viewport after the
navigation and echo replay. No real Pad was deployed or relaunched.

## Run on an independent emulator package

Temporarily change the bundle in `AppScope/app.json5` and its custom file-type
namespace in `entry/src/main/module.json5` and
`entry/src/main/resources/rawfile/arkdata/utd/utd.json5` to
`io.github.storytold.photocraft.pinchtest`. Do not commit those local identity
changes. The runner uses a fresh application process and its default Brush tool.
Build the ARM64 library and default HAP, then the test HAP:

```sh
scripts/dev.sh build
hvigorw assembleHap --mode module -p product=default -p module=entry@ohosTest -p buildMode=debug --no-daemon
hdc -t 127.0.0.1:5555 install -r entry/build/default/outputs/default/entry-default-unsigned.hap
hdc -t 127.0.0.1:5555 install -r entry/build/default/outputs/ohosTest/entry-ohosTest-unsigned.hap
hdc -t 127.0.0.1:5555 shell aa start -a EntryAbility -b io.github.storytold.photocraft.pinchtest
```

Let first-start document-directory initialization finish before running the test.
Pre-start the UiTest daemon because the test runner imports UiTest before the
AbilityDelegator is available:

```sh
python3 scripts/test_pinch_simulator.py --target 127.0.0.1:5555 --bundle io.github.storytold.photocraft.pinchtest
```

Expected result: `TestFinished-ResultCode: 0` from the runner, followed by
`PASS` from the Python verifier with revisions `[1,1,1,2]`.
The runner creates `/data/local/tmp/photocraft-pinch-before.png` and
`/data/local/tmp/photocraft-pinch-after.png` on the simulator, plus
`/data/local/tmp/photocraft-pinch-pen.png` after the positive-control stroke. Export these with
`hdc file recv` for visual inspection.
