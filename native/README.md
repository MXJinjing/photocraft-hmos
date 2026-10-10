# HarmonyOS native self-drawing host

The experimental `codex/hmos-native-selfdraw` branch runs PhotoCraft as a Rust native library inside the HAP. XComponent supplies a NativeWindow; wgpu's GLES/EGL backend and egui-wgpu draw the original editor. The document compositor remains on the CPU in this first iteration.

The adapter is a separate Cargo workspace, outside the upstream subtree. Upstream algorithms and documents are reused through path dependencies. Its lockfile pins the same dependency versions as the upstream lockfile; do not regenerate it wholesale when updating upstream. Both workspaces use the verified wgpu-hal path patch.

## Build

Install a Rust toolchain satisfying upstream's rust-version and both targets:

```sh
rustup target add aarch64-unknown-linux-ohos
scripts/dev.sh build
scripts/dev.sh run
```

`scripts/build_native.sh` builds the ARM64 static library. hvigor/CMake links it into `libphotocraft.so`. Local DevEco debug signing remains in the ignored root build profile. `scripts/verify_native.py <HAP>` verifies the ARM64 ELF architecture and rejects packaged HTML, JavaScript or WASM.

The scripts read the existing ignored `scripts/dev.local.env`. Set `PHOTOCRAFT_NATIVE_SDK` to the SDK's `openharmony/native` directory when necessary. `PHOTOCRAFT_NATIVE_TARGETS` can select one Rust target for standalone build experiments; a full HAP build requires both. `PHOTOCRAFT_RUST_PROFILE` is release by default; standalone debug libraries do not satisfy the default CMake release path.

Native Rust and ArkTS/C++ changes require rebuilding and installing the HAP. `scripts/dev.sh` supports `build`, `run` and `launch`; every entry uses the native Rust host. Save work before relaunch or deployment, which restart the app. File Wants are forwarded to the current native editor. Imported upstream Web sources remain in the subtree history and are outside the HarmonyOS build/development workflow.

## Ownership and system services

One Rust worker owns the editor, egui context, device and renderer. XComponent callbacks copy input to its channel. A native window reference is acquired in the surface callback and released after the worker drops its surface. Device, renderer and texture cache survive surface detach/rebind.

Only requested frames use Native VSync. ArkTS receives system-service requests via N-API's thread-safe callback. The Rust worker waits for picker completion; the ArkTS main thread remains available. The writer returns success only after all bytes are written, flushed and the file descriptor is closed. Picker cancellation and write errors keep the document dirty. Each Save opens the system picker before encoding. ArkTS associates the selected display name with its URI; the encoder uses the display name and the writer consumes the URI only through system file APIs. Automatic migration of Web storage is outside this experiment.

After each editor frame, the adapter checks `ThemeKind::chrome_css()` and sends a `chrome` notification on the first frame or a theme change. Surface reattachment and foreground activation invalidate that cache. ArkTS validates and stores the colour in `photocraftChrome`, updates the page background, and reuses `SystemBars.ets` to paint the status/navigation areas with contrasting icons. Window-stage completion and foreground restoration use the stored colour, so they cannot overwrite an already received theme with the loading colour. The XComponent continues to lay out inside the safe area.

Unsafe pointer/handle interop is isolated here; the upstream workspace continues to forbid unsafe code. Input buffers are copied synchronously, capped at 256 MiB, and never retained across FFI calls. Native window teardown cancels outstanding service waiters. Suspending for a system picker does not cancel its save. A close request is queued until the current picker resolves. egui-generated Close commands (including its built-in Ctrl+Q) are returned as viewport close events before termination, allowing the original unsaved-document guard to veto them.

## First-iteration limits

Chinese UI uses the device's HarmonyOS Sans fonts and system locale. The native input bridge forwards pen pressure, tilt and eraser source, distinguishes fingers, and subscribes to the existing HarmonyOS pen gesture adapter. Pen contact takes priority over a finger contact. Screen contacts retain their IDs and egui touch phases for two-finger centroid pan/pinch. Finger contact hides tool cursors, and pen takeover cancels finger contacts until palms lift. Trackpad axis events carry position, scroll increments and cumulative pinch scale, which the adapter converts to egui zoom ratios. Physical pen and trackpad behaviour still require Pad acceptance; pen hover remains deferred.

The custom-editor IME adapter binds both egui text fields and canvas Type sessions. Opening Help automatically focuses its search field and requests the keyboard once; other fields request it on a tap, and canvas text sessions request it when editing starts. Composition, commits, UTF-16 selection and deletion are routed to the corresponding editor, with focus tokens rejecting stale callbacks. ArkUI keyboard avoidance resizes the native surface so egui lays out within the available height. Input text is excluded from bridge notification logs.

The v0.5.0 menu comparison isolated Help's focus loss to opening on press before the title release. Help now opens on a completed click and closes on outside clicks or an explicit command selection. Its search retains focus through keyboard resize and editor taps, allowing the native IME bridge to show the keyboard automatically. Other menus retain upstream press-to-open behaviour.

System clipboard, automatic recovery and large-image performance acceptance are deferred. Fonts and signing secrets are never committed.

## Touch regression checks

Run `python3 scripts/test_native_touch.py` for the actual C++ Touch/Axis callbacks, and `cargo test --locked --manifest-path native/rust/Cargo.toml --lib` for egui touch navigation, no-edit navigation, brush cursor visibility, pen priority and existing host services. XComponent `isPressed` is false during simulator MOVE events; contact lifetime uses touch phases instead.

## Stylus control and mouse cursors

The title-bar Stylus control, immediately left of Discord, appears after the platform first reports a connected pen. The feed latches that discovery for the current app lifetime, so disconnecting the pen keeps its settings accessible; a pen contact alone does not count as a connection report.

The native worker forwards the final egui cursor to ArkUI, preserving the UI pass cursor when merging the earlier logic output. `None` hides the system pointer while egui draws its brush tip or custom tool cursor; other cursor icons map to HarmonyOS text, crosshair, hand, resize and zoom styles. Blur, page disposal and background activation restore pointer visibility, and attachment/foreground changes invalidate the worker cache. Run `node scripts/test_native_cursor.cjs <typescript module path>` for the ArkTS adapter checks. Physical mouse/pen behaviour still requires Pad acceptance.
