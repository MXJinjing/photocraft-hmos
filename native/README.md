# HarmonyOS native self-drawing host

The experimental `codex/hmos-native-selfdraw` branch runs PhotoCraft as a Rust native library inside the HAP. XComponent supplies a NativeWindow; wgpu's GLES/EGL backend and egui-wgpu draw the original editor. The document compositor remains on the CPU in this first iteration.

The adapter is a separate Cargo workspace, outside the upstream subtree. Upstream algorithms and documents are reused through path dependencies. Its lockfile pins the same dependency versions as the upstream lockfile; do not regenerate it wholesale when updating upstream. Both workspaces use the verified wgpu-hal path patch.

## Build

Install a Rust toolchain satisfying upstream's rust-version and both targets:

```sh
rustup target add aarch64-unknown-linux-ohos x86_64-unknown-linux-ohos
scripts/dev.sh build
scripts/dev.sh run
```

`scripts/build_native.sh` builds ARM64 and x86_64 static libraries. hvigor/CMake links them into `libphotocraft.so`. Local DevEco debug signing remains in the ignored root build profile. `scripts/verify_native.py <HAP>` verifies the two ELF architectures and rejects packaged HTML, JavaScript or WASM.

The scripts read the existing ignored `scripts/dev.local.env`. Set `PHOTOCRAFT_NATIVE_SDK` to the SDK's `openharmony/native` directory when necessary. `PHOTOCRAFT_NATIVE_TARGETS` can select one Rust target for standalone build experiments; a full HAP build requires both. `PHOTOCRAFT_RUST_PROFILE` is release by default; standalone debug libraries do not satisfy the default CMake release path.

Rust changes require rebuilding and installing the HAP. There is no Trunk server, web reload or hdc port forwarding. Upstream's Web target still builds separately for regression checks.

## Ownership and system services

One Rust worker owns the editor, egui context, device and renderer. XComponent callbacks copy input to its channel. A native window reference is acquired in the surface callback and released after the worker drops its surface. Device, renderer and texture cache survive surface detach/rebind.

Only requested frames use Native VSync. ArkTS receives system-service requests via N-API's thread-safe callback. The Rust worker waits for picker completion; the ArkTS main thread remains available. The writer returns success only after all bytes are written, flushed and the file descriptor is closed. Picker cancellation and write errors keep the document dirty. Each Save opens the system picker before encoding. ArkTS associates the selected display name with its URI; the encoder uses the display name and the writer consumes the URI only through system file APIs. Automatic migration of Web storage is outside this experiment.

Unsafe pointer/handle interop is isolated here; the upstream workspace continues to forbid unsafe code. Input buffers are copied synchronously, capped at 256 MiB, and never retained across FFI calls. Native window teardown cancels outstanding service waiters. Suspending for a system picker does not cancel its save. A close request is queued until the current picker resolves. egui-generated Close commands (including its built-in Ctrl+Q) are returned as viewport close events before termination, allowing the original unsaved-document guard to veto them.

## First-iteration limits

Chinese UI uses the device's HarmonyOS Sans fonts and system locale. Chinese IME, system clipboard, full pen pressure/tilt/hover integration, automatic recovery and large-image performance acceptance are deferred. Fonts and signing secrets are never committed.
