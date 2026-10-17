# Bottom navigation indicator (小横条) inset

Status: experimental, needs on-device acceptance (branch `feat/hmos-bottom-inset`).

## Behaviour

On tablets the system reserves a strip at the bottom of the screen for the navigation indicator.
Before this change the page laid out above it, so the strip stayed an empty system-coloured band.
Now the editor surface extends underneath the indicator and the editor keeps its own controls
above it:

- The status bar is unchanged: the page still avoids it (only the **bottom** edge is extended).
- Toolbars, panels, dialogs, windows and the canvas (including its scrollbars and click targets)
  lay out inside `egui::Context::content_rect()`, i.e. above the indicator.
- The strip under the indicator is painted by the host: the status-bar colour while the status bar
  is shown, the canvas backdrop otherwise, black in full-screen mode.

## How it works

ArkTS (`entry/src/main/ets`)

- `pages/Index.ets`: the root `Stack` and the `XComponent` use
  `expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.BOTTOM])`. `setWindowLayoutFullScreen` is
  deliberately not used: it changes layout for the whole app (status bar, freeform title bar, window
  jumps), while `expandSafeArea` only touches the bottom edge of this page.
  `setKeyboardAvoidMode(RESIZE)` is unchanged (`SafeAreaType.KEYBOARD` is not requested).
- `NavigationInset.ets` (`NavigationIndicatorInset`): reads
  `getWindowAvoidArea(TYPE_NAVIGATION_INDICATOR).bottomRect.height` and the window height, listens
  to `avoidAreaChange` (navigation-indicator type only) and `windowSizeChange`, re-reads once after
  250 ms for late transactions, and publishes only changes. Nothing is hard-coded: floating windows,
  split screen and fullscreen report different values. A failed query keeps the last value.
- Message `bottom_inset` (through `native.input`), **physical pixels**: `dy` indicator height,
  `dx` window height, `y` top of the editor surface in the window. The window geometry is resent on
  every native `ready`.

Rust (`native/rust/src/safe_inset.rs`)

- The worker stores the geometry and, each frame, computes the real overlap
  `clamp(surface_top + surface_height - (window_height - indicator), 0, indicator)` from the actual
  surface height. The inset therefore exists only while the surface really reaches under the
  indicator: it is 0 in floating windows and split screen, when the keyboard has shrunk the page
  (RESIZE), and if the platform did not extend the component.
- Pixels become points with the same `native scale * UI zoom` used for the title-bar geometry, are
  capped at 25 % of the window height, and are handed to egui as `RawInput::safe_area_insets`
  (always `Some`, because `None` means "same as last frame").
- `safe_inset::host_ui` lays the upstream editor out in `content_rect()` and fills the strip. The
  upstream subtree is not modified.

## Known risk: strokes that start at the bottom edge

The system swipe-up gesture (home / recents) starts in the indicator strip. egui no longer places
controls or the canvas there, but pen or finger strokes that start at the very bottom of the screen
can still be taken by the system gesture, and the pointer events in the strip are not forwarded to
any canvas. Start strokes slightly above the screen edge.

## Verification

- `cargo test --lib` (native/rust): inset conversion, overlap rules, and an egui layout test that
  checks the status bar and panels end at the inset and the host strip fills the rest.
- `node scripts/test_navigation_inset.cjs`: avoid-area / size listeners, change-only publishing,
  failure retention, resend, cleanup.
- Not verifiable without a device: whether the `XComponent` surface really extends under the
  indicator, and the avoid-area numbers. See the checklist in the pull request or branch report:
  tablet default, floating window, split screen, keyboard, bottom strokes and swipe-up.
