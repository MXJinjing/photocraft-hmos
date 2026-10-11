# HarmonyOS native self-drawing host

This repository runs PhotoCraft v0.6.0 as a Rust native library inside the HAP. XComponent supplies a NativeWindow; wgpu's GLES/EGL backend and egui-wgpu draw the original editor. The document compositor remains on the CPU in this first iteration.

The adapter is a separate Cargo workspace, outside the upstream subtree. Upstream algorithms and documents are reused through path dependencies. Its lockfile pins the same dependency versions as the upstream lockfile; do not regenerate it wholesale when updating upstream. Both workspaces use the verified wgpu-hal path patch.

## Build

Install a Rust toolchain satisfying upstream's rust-version and the ARM64 target:

```sh
rustup target add aarch64-unknown-linux-ohos
scripts/dev.sh build
scripts/dev.sh run
```

`scripts/build_native.sh` builds the ARM64 static library. hvigor/CMake links it into `libphotocraft.so`. Local DevEco debug signing remains in the ignored root build profile. `scripts/verify_native.py <HAP>` verifies the ARM64 ELF architecture and rejects packaged HTML, JavaScript or WASM.

After each successful Rust build, `scripts/build_native.sh` runs `scripts/clean_rust_debug.py` to clear `native/rust/target/debug`, including host dependencies, incremental data and fingerprints. The Cargo lock file, ARM64 outputs and release cache are preserved; an active Cargo lock skips cleanup. This applies to `dev.sh`, offline packaging and CI. Direct `cargo test` commands do not trigger cleanup. Clearing the cache means host build dependencies and local tests need recompilation on their next run.

The scripts read the existing ignored `scripts/dev.local.env`. Set `PHOTOCRAFT_NATIVE_SDK` to the SDK's `openharmony/native` directory when necessary. `PHOTOCRAFT_NATIVE_TARGETS` accepts only `aarch64-unknown-linux-ohos`; local and CI HAPs both contain only ARM64 libraries. `PHOTOCRAFT_RUST_PROFILE` is release by default; standalone debug libraries do not satisfy the default CMake release path.

Native Rust and ArkTS/C++ changes require rebuilding and installing the HAP. `scripts/dev.sh` supports `build`, `run` and `launch`; every entry uses the native Rust host. Save work before relaunch or deployment, which restart the app. File Wants are forwarded to the current native editor. Imported upstream Web sources remain in the subtree history and are outside the HarmonyOS build/development workflow.

## Ownership and system services

One Rust worker owns the editor, egui context, device and renderer. XComponent callbacks copy input to its channel. A native window reference is acquired in the surface callback and released after the worker drops its surface. Device, renderer and texture cache survive surface detach/rebind.

Only requested frames use Native VSync. ArkTS receives system-service requests via N-API's thread-safe callback. The Rust worker waits for picker completion; the ArkTS main thread remains available. The writer returns success only after all bytes are written, flushed and the file descriptor is closed. Picker cancellation and write errors keep the document dirty. Each Save opens the system picker before encoding. ArkTS associates the selected display name with its URI; the encoder uses the display name and the writer consumes the URI only through system file APIs. Automatic migration of Web storage is outside this experiment.

After each editor frame, the adapter checks `ThemeKind::chrome_css()` and sends a `chrome` notification on the first frame or a theme change. Surface reattachment and foreground activation invalidate that cache. ArkTS validates and stores the colour in `photocraftChrome`, updates the page background, and reuses `SystemBars.ets` to paint the status/navigation areas with contrasting icons. Window-stage completion and foreground restoration use the stored colour, so they cannot overwrite an already received theme with the loading colour. The XComponent continues to lay out inside the safe area.

Unsafe pointer/handle interop is isolated here; the upstream workspace continues to forbid unsafe code. Input buffers are copied synchronously, capped at 256 MiB, and never retained across FFI calls. Native window teardown cancels outstanding service waiters. Suspending for a system picker does not cancel its save. A close request is queued until the current picker resolves. egui-generated Close commands (including its built-in Ctrl+Q) are returned as viewport close events before termination, allowing the original unsaved-document guard to veto them.

## First-iteration limits

Chinese UI uses the device's HarmonyOS Sans fonts and system locale. The native input bridge forwards pen pressure, tilt and eraser source, distinguishes fingers, and subscribes to the existing HarmonyOS pen gesture adapter. Pen contact takes priority over a finger contact. Screen contacts retain their IDs and egui touch phases for two-finger centroid pan/pinch. Finger contact hides tool cursors, and pen takeover cancels finger contacts until palms lift. Pen activity drops any finger-owned primary pointer in place (not at the pen tip) so resting fingers cannot rubber-band ink to the tip; two-or-more finger snapshots never synthesize a tool primary press, and a second finger cancels a one-finger primary so pan/pinch cannot deposit stroke points. Trackpad axis events carry position, scroll increments and cumulative pinch scale, which the adapter converts to egui zoom ratios. Physical pen and trackpad behaviour still require Pad acceptance. Stylus hover uses ArkUI `onHoverMove` (API 15+) on the editor XComponent: component-local vp is converted to physical pixels and forwarded as a pen `pointer` move with pressure 0, so egui can draw the brush tip without starting a stroke. Proximity leave (ArkUI `onHover` and the XComponent Hover callback) uses `hover_leave`; focus loss still uses `blur`. After a stroke the tip is held at the last contact point: post-lift hover/mouse echoes are rewritten to that point until the next pen-down or focus blur, so the tip does not jump back to the pre-stroke hover start. HoverEvent does not expose tilt, eraser end, or proximity distance; those remain contact-only.

The custom-editor IME adapter binds both egui text fields and canvas Type sessions. Opening Help automatically focuses its search field and requests the keyboard once; other fields request it on a tap, and canvas text sessions request it when editing starts. Composition, commits, UTF-16 selection and deletion are routed to the corresponding editor, with focus tokens rejecting stale callbacks. ArkUI keyboard avoidance resizes the native surface so egui lays out within the available height. Input text is excluded from bridge notification logs.

The v0.5.0 menu comparison isolated Help's focus loss to opening on press before the title release. Help now opens on a completed click and closes on outside clicks or an explicit command selection. Its search retains focus through keyboard resize and editor taps, allowing the native IME bridge to show the keyboard automatically. Other menus retain upstream press-to-open behaviour.

System clipboard, automatic recovery and large-image performance acceptance are deferred. Fonts and signing secrets are never committed.

## Touch regression checks

Run `python3 scripts/test_native_touch.py` for the actual C++ Touch/Axis callbacks, and `cargo test --locked --manifest-path native/rust/Cargo.toml --lib` for egui touch navigation, no-edit navigation, brush cursor visibility, pen priority and existing host services. XComponent `isPressed` is false during simulator MOVE events; contact lifetime uses touch phases instead.

## Stylus control and mouse cursors

The title-bar Stylus control, immediately left of Discord, appears after the platform first reports a connected pen. The feed latches that discovery for the current app lifetime, so disconnecting the pen keeps its settings accessible; a pen contact alone does not count as a connection report.

The native worker forwards the final egui cursor to ArkUI, preserving the UI pass cursor when merging the earlier logic output. `None` hides the system pointer while egui draws its brush tip or custom tool cursor; other cursor icons map to HarmonyOS text, crosshair, hand, resize and zoom styles. Blur, page disposal and background activation restore pointer visibility, and attachment/foreground changes invalidate the worker cache. Run `node scripts/test_native_cursor.cjs <typescript module path>` for the ArkTS adapter checks. Physical mouse/pen behaviour still requires Pad acceptance. Run `node scripts/test_pen_hover_preview.cjs <typescript module path>` for the hover encoder checks.
