# Integrated title-bar geometry recovery

2026-10-10: fixed loss of caption-button clearance during window transitions.

The host previously cleared its inset on a failed query or a zero rectangle, and rejected valid rectangles unless the window status was floating/maximized. Status and geometry can update independently. The host now accepts valid caption rectangles regardless of a lagging status, retains its last valid inset during floating/maximized zero-geometry transitions and query failures, and refreshes at 50/200/600/1200 ms after status/size/foreground changes. Timers and listeners are removed at window-stage destruction.

The native ready notification resends current geometry. Rust retains physical geometry and converts it each frame using the current egui zoom factor.

2026-10-11: fixed missing clearance when the app starts outside freeform and later enters it.

`setWindowDecorHeight` / `setWindowDecorVisible` / `setWindowTitleMoveEnabled` often throw outside free multi-window (abnormal window state). Those calls used to sit in the same `try` as listener registration, so a fullscreen start could leave `WindowChrome` with no status/size/rect listeners. Entering freeform later never refreshed `photocraftTitleRight`, and the native caption buttons covered the top-bar controls.

Decor setup is now best-effort and separate from listening. Status and size changes re-apply decor before refreshing geometry, so a runtime freeform entry recovers the inset even when startup decor failed.

2026-10-11 (compile fix): PR #4 briefly typed the shared layout callback as `WindowStatusType` and reused it for `windowSizeChange`. ArkTS overload resolution then rejected `on/off('windowSizeChange', …)` (error mentioning `freeWindowModeChange`). Status and size now use separate callbacks (`Callback<WindowStatusType>` vs `Callback<Size>`) that both re-apply decor and refresh geometry.

Validation:

- `scripts/test_window_chrome.cjs`: passed mocked transition-zero, query-error, delayed-recovery, visible-caption/lagging-status, non-floating empty geometry, lifecycle cleanup, and freeform-entry-after-fullscreen (decor-throw and size-driven) regressions.
- Dual-target Rust release build and signed HAP build passed; `verify_native.py` passed (2026-10-10 validation).
- Installed on real device `3GLUN25314G05064` after confirming the app had no open documents (2026-10-10).
- Floating-window and restored foreground large-window screenshots show workspace/stylus controls clear of the native caption buttons. Device log reports a 139×37 vp caption rectangle and 147 vp reserved width.
- The attempted maximize-button interaction returned to the desktop; this does not establish a complete maximize/recover interaction test. Intermittent failures across repeated transitions remain to be observed.
- 2026-10-11 freeform-entry fix: CLI unit tests only; real-device verification deferred to the user.

Local evidence (not committed): `work/titlebar-recovery-after.jpeg`, `work/titlebar-recovery-restored.jpeg`, `work/titlebar-recovery-hilog.log`, `work/titlebar-recovery-build.log`.
