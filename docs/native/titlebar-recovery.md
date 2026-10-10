# Integrated title-bar geometry recovery

2026-10-10: fixed loss of caption-button clearance during window transitions.

The host previously cleared its inset on a failed query or a zero rectangle, and rejected valid rectangles unless the window status was floating/maximized. Status and geometry can update independently. The host now accepts valid caption rectangles regardless of a lagging status, retains its last valid inset during floating/maximized zero-geometry transitions and query failures, and refreshes at 50/200/600 ms after status/size/foreground changes. Timers and listeners are removed at window-stage destruction.

The native ready notification resends current geometry. Rust retains physical geometry and converts it each frame using the current egui zoom factor.

Validation:

- `scripts/test_window_chrome.cjs`: passed mocked transition-zero, query-error, delayed-recovery, visible-caption/lagging-status, non-floating empty geometry and lifecycle cleanup regressions.
- Dual-target Rust release build and signed HAP build passed; `verify_native.py` passed.
- Installed on real device `3GLUN25314G05064` after confirming the app had no open documents.
- Floating-window and restored foreground large-window screenshots show workspace/stylus controls clear of the native caption buttons. Device log reports a 139×37 vp caption rectangle and 147 vp reserved width.
- The attempted maximize-button interaction returned to the desktop; this does not establish a complete maximize/recover interaction test. Intermittent failures across repeated transitions remain to be observed.

Local evidence (not committed): `work/titlebar-recovery-after.jpeg`, `work/titlebar-recovery-restored.jpeg`, `work/titlebar-recovery-hilog.log`, `work/titlebar-recovery-build.log`.
