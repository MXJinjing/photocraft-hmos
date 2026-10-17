# Native sub-windows for floating document windows (upstream 0.9.0)

Upstream 0.9.0 shows an extra document window with
`ctx.show_viewport_immediate(ViewportId::from_hash_of(("docwin", id)), …)` from
`canvas::extra_windows` (`Window → Arrange → Float…`, tab dragged off the tab strip).
With egui's default `embed_viewports = true` that call collapses to an in-app
`egui::Window` (MDI): it cannot leave the main window, has no system title bar, and its
GPU canvas shares the root render pass, so it can bleed across sibling windows.

This host instead gives every such viewport **its own HarmonyOS sub-window with its own
XComponent / NativeWindow / wgpu surface**, system decoration included.

## Pieces

```
Rust worker (egui)            C++ NAPI bridge                     ArkTS
─────────────────             ───────────────                     ─────
show_viewport_immediate  ──►  notify("window",{op:"open",win:N})  SubWindowHost.open()
 (embed=false, only in                                              stage.createSubWindowWithOptions(
  extra_windows)                                                      'pc-doc-N',{decorEnabled:true,…})
                                                                    loadContent('pages/SubWindow', {winKey:N})
                                                                    XComponent id 'pc-win-N'
                         ◄──  pc_win_attach(N, NativeWindow,w,h) ◄─ SurfaceCreated (id → key N)
render: shared device/queue/egui_wgpu::Renderer, one Surface per key
input:  Json(...) gets `"win":N` ───────────────────────────────►  per-window input::State
close:  notify {op:"close"} ─────────────────────────────────────► win.destroyWindow()
        ◄── input {kind:"win_close",win:N}  ◄───────────────────  on('windowWillClose') (return true = veto, Rust decides)
```

### Rust (`native/rust/src/viewports.rs`, `render.rs`, `lib.rs`)
* `Context::set_immediate_viewport_renderer` closure (thread-local on the worker). For an
  immediate viewport it: allocates a window key on first sight (emits `open`), builds a
  `RawInput` for that viewport (own size, events, `ViewportInfo`, close request),
  runs `ctx.run_ui(...)` **nested** in the root pass (this is how eframe does it) and
  stashes the `FullOutput` (textures + shapes). The callback is always run exactly once
  per call, even before the native surface exists (egui panics otherwise), using the
  builder's size.
* Embedding is switched off **only while `canvas::extra_windows` runs**
  (`PhotocraftApp::native_doc_windows`, a 6-line upstream hook). Camera Raw and other
  viewports keep today's in-app behaviour.
* Rendering: one `wgpu::Device/Queue/egui_wgpu::Renderer` (so GPU canvas callbacks and
  textures are shared); one `Surface` + `SurfaceConfiguration` + `Arc<Window>` per key.
  Per frame: apply texture deltas in pass order (initial atlas, children, root) → draw each
  child surface → draw root → present → free textures. A failing child surface is dropped
  and the window is asked to close; it never halts the root renderer.
* Input: C++ tags events with `win`. `viewports::Registry` owns an `input::State` per
  window (pointer/touch/pinch/axis); the same touch-navigation / finger-ownership /
  pen-suppression logic as the main window runs per window. Keyboard events from any
  window go to the root queue (app shortcuts live in root `host_logic`).
* Close: system close → `win_close` → next child pass gets `ViewportEvent::Close` →
  `extra_windows` sets `win.open=false` → viewport unused → `close` command → ArkTS
  `destroyWindow()`. A viewport that stops being shown (tab merged back, window
  closed by the app) takes the same final path.
* Lifetime: surface destroyed (app backgrounded) only detaches the target and keeps the
  window entry; surface created re-attaches. Main page `aboutToDisappear`/ability
  `onWindowStageDestroy` close all sub-windows (they are children of the main window).

### C++ (`bridge.cpp`)
Per-`OH_NativeXComponent*` state (`touchId`, `penContact`, `fingerContact`) keyed from the
XComponent id (`photocraft-native` → key 0, `pc-win-N` → key N). `registerWindow(context)`
registers the same callback set from `onLoad(context)` on the sub-window's XComponent.
`pc_win_attach/resize/detach` are the per-window equivalents of `pc_attach/…`.

### ArkTS
`SubWindowHost.ets` (create/title/resize/destroy; `windowWillClose` veto), `pages/SubWindow.ets`
(one XComponent + hover/blur), `Index.ets` routes `window` notifications.

## Platform constraints (API 20 SDK, verified in d.ts)
* `WindowStage.createSubWindowWithOptions(name, {title, decorEnabled, maximizeSupported})` — available.
* `Window.on('windowWillClose')` (API 15) — available; used for the system close button.
* Sub-windows may leave the main window **only in free-window state**; in fullscreen/split
  they are confined to the app window (Huawei sub-window guide).
* *Independent* sub-windows (`zLevelAboveParentLoosened`) are API 26 → not in this SDK.
  API 20 sub-windows always stack above their parent and follow its foreground/background.
* Fallback if "must be fully independent" is required: `multiton` UIAbility instances
  (own mission card). The Rust/C++ per-window layers above are reused unchanged; only
  `SubWindowHost.ets` changes.

## MVP scope / known limits
Document windows only (stylus BLE feed is global and works per window). Not wired per
sub-window: IME composition, drag-and-drop import, pen-contact `host_input` hooks, cursor
icon, per-display DPI (uses the main window's scale), window position restore.
