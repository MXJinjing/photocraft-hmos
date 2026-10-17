# Status bar and bottom navigation indicator (小横条) insets

Status: experimental, needs on-device acceptance (branch `feat/hmos-bottom-inset`).

## Goal

On tablets the system reserves a strip at the bottom of the screen for the navigation indicator
(and one at the top for the status bar). The editor should paint to the screen edges while its
controls (toolbar, panels, canvas, dialogs) stay inside the strips.

## History: what did not work on device

1. `expandSafeArea([SYSTEM], [BOTTOM])` (commit `818d5de2`): drawing-only, does not change layout
   size. The white strip stayed and no inset was reported.
2. `ignoreLayoutSafeArea` + `LayoutPolicy.matchParent` (commit `02b5dc65`, API 20): still no
   avoidance, and the theme fill disappeared. Dropped.
3. Immersive window + `inset = system - alreadyAvoided` for **both** edges (commits through
   `510487ef`): bottom worked on device (screenshot), top did not — menu icons overlapped the
   status bar. Cause: `top = systemTop - contentTop` cancelled the status-bar inset whenever the
   laid-out top was reported near the status-bar height (stale/misreported), while the surface
   still painted underneath it. Fixed by using the full status-bar height for the top inset and
   caching it before fullscreen.

## Current design (deterministic, documented API route)

`setWindowLayoutFullScreen(true)` + `getWindowAvoidArea` + `avoidAreaChange` is the route the
official immersive-window guide describes. It is applied only where it is safe:

- `Immersive.ets` / `EntryAbility.ets`: before `loadContent`, the main window is put in immersive
  layout (the page covers the whole window). Skipped on `2in1` (freeform windows and the integrated
  title bar are handled by `WindowChrome`); if the call fails, the flag `photocraftImmersive`
  stays false and the old avoided layout (no insets) remains.
- `Immersive.ets`: before `setWindowLayoutFullScreen`, caches the status-bar height
  (`TYPE_SYSTEM.topRect` / `TYPE_CUTOUT.topRect`) into `photocraftStatusBarPx`. Some devices report
  a zero top avoid area after fullscreen; the cache is the fallback.
- `NavigationInset.ets`: measures, in physical pixels,
  - **top** = live status-bar height (`max(TYPE_SYSTEM.top, TYPE_CUTOUT.top)`), falling back to the
    pre-fullscreen cache when live is 0. **Never** subtracts the laid-out content top: a stale or
    misreported `contentTop ≈ statusBarHeight` used to cancel the top inset while the surface still
    painted under the status bar (device screenshot: menu icons overlapping the system clock /
    Wi-Fi icons, while the bottom strip was already correct).
  - **bottom** = the larger of `TYPE_NAVIGATION_INDICATOR.bottomRect` and `TYPE_SYSTEM.bottomRect`,
    minus the gap the laid-out surface already leaves at the window bottom. Keyboard
    (`KeyboardAvoidMode.RESIZE`) shrinks that surface and drives the bottom inset to 0.
  It listens to `avoidAreaChange` and `windowSizeChange`, re-reads at 250 ms / 1 s / 2.5 s,
  publishes only changes, and keeps the last value if a query fails. Before the first
  `onAreaChange` the immersive page is assumed to fill the window so both insets exist from the
  first frame; the laid-out box survives `start()`/`stop()`.
- `Index.ets`: root `Stack` is `100% x 100%` with `backgroundColor(chrome)` (theme fill under
  everything). Sends `safe_inset` (`dx` top px, `dy` bottom px), resends on native `ready`.
  `setKeyboardAvoidMode(RESIZE)` is unchanged.
- Rust (`native/rust/src/safe_inset.rs`): converts pixels to points with `native scale * UI zoom`
  (same as the title bar), caps each inset at 25 % of the window height, and passes them as
  `RawInput::safe_area_insets` (always `Some`). `host_ui` lays the editor out in
  `content_rect()` and fills the top/bottom strips with an **opaque** theme colour (chrome colour;
  canvas colour at the bottom when the status bar panel is hidden; black in full-screen mode).
  With zero insets nothing is added. The upstream subtree is not modified.

## Top inset covers the whole host layout

Insets reach egui as `RawInput::safe_area_insets`, and the host lays the *entire* upstream editor
out inside `ctx.content_rect()` (`safe_inset::host_ui`), so title bar, menu, options bar, toolbar,
dock, canvas top edge, windows and modals all start below the status bar. A unit test renders a
frame with a 30 pt top inset and asserts that no rect or text shape other than the host strip
starts inside it.

## Diagnostics

- hilog keyword `PhotoCraft bottom inset`: one line per change with the raw avoid areas
  (`sysTop`, `sysBottom`, `navBottom`), window height, laid-out content box and resulting insets,
  plus `PhotoCraft bottom inset sent: top=… bottom=…`. `PhotoCraft immersive layout enabled` /
  `unavailable` shows whether the window accepted full-screen layout.
- On-screen overlay: `INSET_DEBUG` at the top of `pages/Index.ets` is currently **true** on this
  experimental branch so the next device screenshot shows `sysTopUsed` / `insets=top/bottom`.
  Flip it back to `false` once the status-bar inset is confirmed.

## Known risks

- If `setWindowLayoutFullScreen` is rejected by a device or window mode, nothing changes visually
  (no insets, old layout).
- If the system reports 0 for both bottom avoid-area types on a device, no bottom inset is applied
  although the page is full-window; the overlay/hilog line shows this immediately.
- Strokes that start in the bottom strip: the system swipe-up gesture (home / recents) starts
  there. egui no longer places controls or the canvas in the strip, but strokes that start at the
  very bottom of the screen can still be taken by the system gesture. Start slightly above the edge.

## Verification

- `cargo test --lib`: unit conversion, strips (top and bottom) filled opaque, layout ends at the
  insets, zero insets leave the layout untouched.
- `node scripts/test_navigation_inset.cjs`: inset rule (full layout, already avoided, keyboard,
  floating), SYSTEM-reported bottom bar, avoid/size events, failure retention, immersive helper
  (tablet / 2in1 / failure), source wiring (immersive before `loadContent`, no safe-area
  expansion calls in Index, root background kept).
- `scripts/test_system_bars.cjs` updated: the page now loads right after the immersive promise.
- Not verifiable without a device: whether the window accepts immersive layout, and the real
  avoid-area values.
