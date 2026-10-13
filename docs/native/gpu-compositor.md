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

## CPU fallback and diagnostics

Software adapters use CPU composition. Canvas initialization failures keep the
app alive with a recorded reason. The existing compositor falls back to CPU for
unsupported document features, failed pipelines, unavailable float effect
targets and memory budgets; export still uses the CPU compositor. A document
fallback can keep a GPU canvas alive to display uploaded CPU pixels, and a later
supported refresh can use GPU composition again. Device faults drop the GPU
canvas for the session through the existing recovery path.

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
detach/rebind and CPU fallback. Save open documents before deployment/relaunch.
On a HarmonyOS Pad:

1. Open a small RGB document with ordinary layers. Confirm System Info and
   Preferences > Performance report GPU after its first document refresh.
2. Paint locally. Confirm refreshes use the GPU rectangle path in performance
   diagnostics and compare the resulting image with CPU export.
3. Use an unsupported layer feature (for example advanced blending), then
   confirm CPU composition and the feature's fallback reason. Return to a
   supported document and refresh it; confirm GPU composition resumes.
4. Test a large working set against the memory budget and confirm CPU fallback
   remains usable. Background and restore the app; verify cached documents still
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
