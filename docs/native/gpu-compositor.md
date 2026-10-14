# HarmonyOS native GPU image composition

The native host enables PhotoCraft's existing GPU canvas when its first GLES
hardware adapter is ready. `Render::egui_render_state` clones the existing wgpu
handles and shares the egui renderer through an `Arc<RwLock<Renderer>>`.
`PhotocraftApp::set_wgpu` registers canvas callback resources in that renderer;
native window painting uses the same renderer, device and queue. It does not
create a second graphics device.

Surface detach and rebind retain `Render`, the app and GPU canvas. Only the
NativeWindow reference and surface are replaced. Document textures, compositor
caches and damage-region updates therefore continue through the existing GPU
canvas implementation.

## Document fallback and native GPU failure

The native host requires a hardware GPU adapter. Adapter or canvas initialization
failure stops native drawing and reports an error through ArkUI. The existing
document compositor can still use CPU composition for
unsupported document features, failed pipelines, unavailable float effect
targets and memory budgets; export still uses the CPU compositor. A document
fallback can keep a GPU canvas alive to display uploaded CPU pixels, and a later
supported refresh can use GPU composition again. A native device fault, rendering
error or escaped panic halts presentation until the app is restarted; the host
does not create a CPU NativeWindow producer. See [native failure handling](../../native/recovery.md).

`gpuInfo.canvas` continues to describe canvas availability. The performance
panel and System Info derive **Image compositor** from the existing
`lastRefresh` and `gpuFallback` state. Before the first document refresh, an
initialized GPU canvas reports an unknown compositor. CPU fallback displays its
reason; a successful GPU refresh clears the document fallback. Startup backend
diagnostics are preserved. No control API fields or experimental switches are
added.

## Verification

Run the GPU crate's parity suite, UI GPU canvas diagnostics tests and native
adapter tests. Cross-compile with `aarch64-unknown-linux-ohos`, then build the HAP
and run `scripts/verify_native.py` against that artifact. Use the Rust toolchain
whose sysroot contains the OHOS target; a Homebrew Rust binary does not use
targets installed by rustup.

On a connected simulator, verify startup, a visible document canvas, surface
detach/rebind and the GPU stop/error path. Save open documents before deployment/relaunch.
On a HarmonyOS Pad:

1. Open a small RGB document with ordinary layers. Confirm System Info and
   Preferences > Performance report GPU after its first document refresh.
2. Paint locally. Confirm refreshes use the GPU rectangle path in performance
   diagnostics and compare the resulting image with CPU export.
3. Use an unsupported layer feature (for example advanced blending), then
   confirm CPU composition and the feature's fallback reason. Return to a
   supported document and refresh it; confirm GPU composition resumes.
4. Test a large working set against the document memory budget and confirm CPU
   composition remains usable while the GPU device is healthy. Background and restore the app; verify cached documents still
   draw after surface rebind.

Desktop GPU parity does not establish GLES driver support on a Pad. Simulator
and device acceptance must record the adapter, backend and fallback diagnostics
along with fixed-size screenshots before this feature is considered verified
on HarmonyOS hardware.

### Local results (2026-10-13)

- GPU crate: 25 unit tests, 46 parity tests, 3 device-loss tests and 1 vector-mask
  test passed on macOS. These are desktop GPU results, not HarmonyOS GLES results.
- UI GPU canvas: 10 tests passed, including compositor status and fallback
  diagnostics; GPU status: 5 tests passed.
- Native adapter: 55 tests passed; two existing brush-outline tests failed. Both
  failures also reproduce from an `origin/main` source snapshot with only the
  missing `TextEdit.mask` fixture field supplied.
- OHOS cross-check and ARM64 release library build passed using rustup Rust.
- Unsigned debug HAP (with the release Rust library) built successfully and
  passed `scripts/verify_native.py`.
- Shared UI Wasm check, `cargo metadata --offline --locked`, UI Clippy with
  all targets and warnings denied, and dependency-layer validation passed.
- No simulator or Pad was connected (`hdc list targets` returned `[Empty]`).
  Startup, GLES drawing, lifecycle, local brush refresh and device fallback
  acceptance remain pending; no hardware speedup is claimed.

The branch also repairs test-only call sites left incompatible by the upstream
sync: native `TextEdit.mask`, stylus gesture context arguments and cursor-shape
borrows. Production input behavior is unchanged.

### Device results (2026-10-14)

- Merged `origin/main` at `ccc2cfd2`; ARM64 release Rust library and signed debug
  HAP builds passed. The final HAP SHA-256 was
  `628c7143cd6bf84e011b4b5e78ebb0620aa7802e3c0a35507f0a9f14cf92cf8`.
- Installed and launched on the physical TGR-W10 Pad. Startup reported
  `Native egui / wgpu GLES`; the home screen and Ctrl+N dialog rendered and
  responded normally.
- The opt-in diagnostic HAP exercised device-loss and panic handling on the
  Pad; drawing stopped and the error notification appeared without a process
  crash. The normal HAP was rebuilt without fault-injection features and
  reinstalled afterward. See [failure verification](../../native/recovery.md).
- Native unit tests: 59 passed with the two previously reproduced brush-outline
  failures excluded. The full suite still has those two failures.
- The user confirmed freeform mode, maximization and window resizing on the
  Pad. Original screenshots and the distinction between captured state and
  user-run transition tests are in [the window validation report](titlebar-recovery.md#real-device-validation-2026-10-14).
- The later signed package has a pre-existing bundle/UTD namespace mismatch
  reported by `verify_native.py`; package verification is not reported as a
  pass for this artifact. Full document-feature, large-working-set and repeated
  transition stress acceptance remain open.
