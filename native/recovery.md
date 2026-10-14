# Native GPU failure handling

The HarmonyOS native host uses wgpu GLES/EGL for presentation. GPU failure,
rendering errors and escaped Rust panics all enter one native bridge path:

1. Finish an interrupted egui pass so the context is not left in a poisoned
   state.
2. Drop the GPU renderer and mark the worker halted.
3. Send an ArkUI `error` notification. ArkUI displays the system-level error
   panel over the last system-composited frame.
4. Ignore later frame, resize and surface-attach requests until the process is
   restarted.

The bridge does not bind a CPU NativeWindow and does not retry a failed GPU in
the same process. Restarting PhotoCraft creates a fresh NativeWindow, EGL
surface and wgpu device. This avoids the CPU producer's platform-specific
buffer rotation, freeform multi-window and synchronization failure modes.

The native adapter still owns the NativeWindow reference separately from GPU
objects, and all lifecycle callbacks are serialized through the worker. No
upstream compositor or Rust engine changes are required. Driver process
crashes, forced process termination and allocation aborts cannot be recovered
by Rust `catch_unwind`; the app must be restarted.

## Verification

Native recovery tests cover interrupted egui passes, the halted state, lifecycle
events that must not restart drawing, and dirty-document/history preservation.
Run:

```text
cargo test --offline --locked --manifest-path native/rust/Cargo.toml --lib
```

The two existing brush-outline tests in the upstream input module remain
unrelated failures on this branch; the native recovery tests pass.

For device fault injection, build the ARM64 library with
`--features host-recovery-test` and package it through the normal hvigor/CMake
release-library path. Ctrl+Alt+G marks the GPU device lost and Ctrl+Alt+P
injects a panic in the next egui pass. Both must leave the process alive, emit
the error notification, stop subsequent frame submissions, and require an app
restart to return to the GPU path. Normal packaging omits the test feature.

On 2026-10-14, the diagnostic HAP was tested on physical TGR-W10
`192.168.3.221:34139` (Maleoon 910, GLES 3.2). Both injected device loss and
panic produced the stop/error path without a process crash. The final
feature-free signed HAP was rebuilt and installed afterward; startup logged
`Native egui / wgpu GLES`.
