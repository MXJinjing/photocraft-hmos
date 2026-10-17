# Fullscreen toggle (host-layer)

The bottom status strip in the editor is drawn by upstream egui. The HarmonyOS host overlays a **全屏 / 退出全屏** button on that strip instead of changing `upstream/photocraft`.

## Behaviour

- Tap **全屏**: `setWindowLayoutFullScreen(true)` then `setWindowSystemBarEnable([])` so the status bar and navigation indicator hide and the XComponent grows into the former avoid areas.
- Tap **退出全屏**: restore `setWindowLayoutFullScreen(false)` and `setWindowSystemBarEnable(['status', 'navigation'])`, then repaint theme chrome via `SystemBars.ets`.
- State is stored in `AppStorage` (`photocraftFullscreen`). `EntryAbility.onForeground` re-asserts immersive mode after resume.
- Existing `setKeyboardAvoidMode(RESIZE)` and system-bar colouring stay unchanged; colouring still runs while bars are hidden and is reapplied on exit.

## 2in1

The control is **hidden** when `deviceInfo.deviceType === '2in1'`. Freeform / multi-window title decoration is owned by `WindowChrome`; immersive fullscreen would fight that path.

## Risk

After entering fullscreen, strokes near the bottom edge can conflict with the system bottom swipe (home / recent apps). Prefer starting ink slightly above the status strip, or exit fullscreen before relying on edge gestures.

## Checks

```sh
node scripts/test_fullscreen_toggle.cjs /home/box/.local/lib/node_modules/typescript
node scripts/test_system_bars.cjs /home/box/.local/lib/node_modules/typescript
```

No HAP build in this environment (Java / full OHOS packaging toolchain unavailable). Pad acceptance still required for enter/exit, status-bar hide, 2in1 absence of the button, and keyboard resize while fullscreen.
