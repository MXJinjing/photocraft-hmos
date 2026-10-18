# Optional window full screen

Default behaviour is **unchanged**: the window keeps its status bar and navigation indicator, `SystemBars.ets` keeps colouring them with the theme, and the page lays out inside the safe area. Full screen is something the user switches on.

## Entry points (both in egui, HarmonyOS only)

| Where | What |
| --- | --- |
| Status bar, far right | Icon button. Shows **fullscreen-enter** (four diagonal corner arrows pointing **out**) when not full screen, **fullscreen-exit** (arrows pointing **in**) when it is. Set apart from the two panel buttons by an 8 px gap. Distinct from *Only show canvas* (`canvas-only`: hollow centre square + four L-shaped corner brackets — a viewfinder, not arrows). |
| Window menu | **Enter Full Screen / Exit Full Screen**, directly below the *Theme* submenu. |

Both exist only when the host installs `Services::window_fullscreen` (`upstream/.../window_fullscreen.rs`); every other platform and the tests see no change.

## Flow

1. egui click → `bridge::notify(0, "fullscreen", "enter" | "exit")`.
2. `Index.ets` → `requestFullscreen()` (`Fullscreen.ets`): `setWindowLayoutFullScreen(true)` + `setWindowSystemBarEnable([])`; exit restores `setWindowLayoutFullScreen(false)` + `['status', 'navigation']` and repaints the theme colour through `SystemBars.ets`.
3. ArkTS → Rust `native.input({"kind":"fullscreen_state","text":"on"|"off"})` after **every** request (also on failure or refusal). Rust only mirrors this state, so the icon and menu text never get ahead of the window.
4. `FullscreenSync` (EntryAbility) listens to `windowStatusChange`: if the system turns the window into a floating/split window while full screen is on, it leaves full screen and reports `off`. On foreground it re-hides the bars if full screen is on.

Full screen draws edge to edge: no inset padding is applied. `setKeyboardAvoidMode(RESIZE)` is untouched.

## 2in1

Not offered: `fullscreenAvailable()` is false, ArkTS sends `fullscreen:false` in the native config, Rust installs no service, and neither the button nor the menu item appears. Freeform windows share the title strip with `WindowChrome`; immersive layout would fight it. Floating and split windows on tablets refuse to enter full screen for the same reason.

## Risk

While full screen, strokes that start at the very bottom (or top) edge can be taken by the system swipe gestures (home / recents / notification shade). Leave full screen or start a little inside the canvas.
