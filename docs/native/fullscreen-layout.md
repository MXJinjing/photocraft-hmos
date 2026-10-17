# Immersive fullscreen layout (host-layer)

On phone and tablet the HarmonyOS host starts in immersive fullscreen: the page fills the whole window and the status bar / navigation indicator are hidden. The XComponent and egui paint edge-to-edge. There is **no** ArkTS→Rust safe-area inset padding.

## Behaviour

1. Before `loadContent`, `EntryAbility` calls `setWindowLayoutFullScreen(true)` then `setWindowSystemBarEnable([])` (`Immersive.ets`).
2. `onForeground` re-asserts the same sequence so a system restore of the bars while backgrounded does not stick.
3. Theme chrome still updates the **window background** (and 2in1 caption-button style) through `SystemBars.ets`. Status / navigation bar dyeing via `setWindowSystemBarProperties` is not used.
4. Existing `setKeyboardAvoidMode(RESIZE)` is unchanged: the keyboard still shrinks the surface.

## 2in1

Immersive fullscreen is **skipped** when `deviceInfo.deviceType === '2in1'`. Freeform / multi-window title decoration is owned by `WindowChrome`; hiding system bars or forcing layout-fullscreen would fight that path. On 2in1 the system bars stay visible under system styling.

## Risk

With the navigation indicator hidden, strokes near the bottom edge can conflict with the system bottom swipe (home / recent apps). Prefer starting ink slightly above the status strip when that gesture is reliable on the device.

## Checks

```sh
node scripts/test_immersive.cjs /home/box/.local/lib/node_modules/typescript
node scripts/test_system_bars.cjs /home/box/.local/lib/node_modules/typescript
node scripts/test_host_mode.cjs /home/box/.local/lib/node_modules/typescript
```

No HAP build in this environment (Java / full OHOS packaging toolchain unavailable). Pad acceptance still required for edge-to-edge paint, hidden bars, 2in1 skip, keyboard resize, and bottom-edge stroke vs system swipe.
