# HarmonyOS document storage and keyboard verification

2026-10-10

## Final behavior

- Opening a PSD/PSB or image reads its bytes without creating a working copy.
- An unmodified PSD/PSB's ordinary Save does not allocate a file or show a naming dialog.
- The first ordinary Save of an edited Photoshop document, image, or new document asks for an editable PSD name in an ArkUI dialog. Cancellation leaves the document unsaved. The destination is always the system-returned `Download/<bundle>/` directory; existing names receive numeric suffixes before the extension.
- Later ordinary saves reuse the document's selected working target. Save As uses the existing system location picker and does not change the ordinary-save target.
- The system file manager can browse the download directory. Preferences and recovery/staging files remain in the sandbox.

The directory API is `DocumentViewPicker.save` with `DocumentPickerMode.DOWNLOAD`, available since API 12. The application itself retains its existing API 20 minimum. No API 26 `shareFiles` configuration is used. DOWNLOAD ignores other picker options and skips the picker UI; `defaultFilePathUri` only sets a starting location and cannot enforce a destination. Consequently, the first-save name confirmation uses ArkUI instead of offering an unrestricted location picker.

Reference: [OpenHarmony save-user-file guide](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/file-management/save-user-file.md#download模式保存文件).

## Keyboard fix

egui 0.36 `Context.run_logic` updates viewport metadata but leaves UI input untouched. The old native frame ran shortcut handling there, before `run_ui` ingested the current keyboard events. A regression test failed with Brush still selected after E. The host now runs editor logic inside the first UI pass, once per frame even during a layout retry. This exposes current keys, retains modifier events across frames, and produces one output containing logic and UI viewport commands. Initial style/font setup runs separately before system fonts are registered. XComponent takes focus on load and touch; naming dialogs and file pickers restore editor focus on completion.

## Public-file writes

API 20 permits public document files to be opened through CoreFileKit, but rejects direct Rust file creation and public path rename/unlink. The adapter therefore stages the export in the sandbox, backs up the existing file, then publishes through a granted CoreFileKit file descriptor. Short writes are handled; publishing failures restore the backup. If rollback also fails, the backup is retained and its path is reported. This is not an atomic public-directory rename; a process/power interruption during publishing can leave the sandbox backup for recovery.

## Verification

- Native unit tests: 20 passed, including actual Harmony key JSON -> input state -> editor frame -> E / Ctrl+S, held Ctrl, release, single dispatch, and unsaved close veto.
- UI service tests: 5 passed, including clean PSD/PSB save without allocation, first edited save, cancellation, failed write preserving dirty state, and Save As versus ordinary Save destinations.
- ArkTS source tests: name dialog, API 12 directory acquisition, collisions, short writes, source preservation, staging failures, publish rollback and cleanup passed.
- Both OHOS Rust architectures built; native HAP verification passed. OHOS library clippy and upstream UI all-target clippy passed with warnings denied. Layer validation and wasm checks passed.
- API 20 simulator: a separate `io.github.storytold.photocraft.workflowtest` bundle protected the original application's unsaved document. Ctrl+O opened the system picker, E selected Eraser, Ctrl+S opened the editable same-stem PSD dialog, cancellation returned focus, confirmed save succeeded in the app download directory, and a later Ctrl+S reused that path without another dialog. Chinese font rendering was checked after moving the input processing pass. With the final delay-save behavior, opening a PSD followed by Ctrl+S showed no naming dialog or write; Ctrl+J modified a layer and the next Ctrl+S opened first-save naming. The edited PSD saved as `native-selfdraw1.psd` when the original name already existed. Initial Ctrl+O worked without a preceding canvas click; Ctrl+Shift+S opened the external Save As picker.
- Earlier full UI suite: 883 passed, 3 ignored, 1 failed because the GPU-based shortcut-capture test could not find an adapter in this shell. The targeted workflow tests do not require a GPU.

No real-device deployment or original simulator-app relaunch was performed.

Screenshot evidence (2880 × 1920): [clean PSD viewing](document-view-psd.jpeg), [edited PSD first save with collision suffix](document-save-edited-psd.jpeg), [E after cancelling Save As](keyboard-after-saveas.jpeg).

## Save As format labels follow-up

The HarmonyOS picker previously provided only `PhotoCraft|<suggested extension>`, so a PSD export appeared as PhotoCraft and no alternate formats were visible. `saveFormats.ets` now supplies the desktop host's formats: PSD, PSB, .pcraft, PNG, JPEG, WebP, TIFF, Targa and OpenEXR. The suggested extension stays first; JPEG/TIFF aliases are normalized. Ordinary Save still uses the local PSD workflow. Format source tests and a dual-architecture HAP build passed. The API 20 simulator displayed all nine formats: [format dropdown](save-formats.jpeg).

## Saved document title follow-up (2026-10-10)

Successful Save / Save As now updates the active document name from the actual destination basename, after the writer succeeds. Both the title bar and document tab follow the chosen name, including collision suffixes; cancelled selections and failed writes preserve the previous name. Existing ordinary-save destination caching is preserved.

Verification: UI unit suite 887 passed / 3 ignored; native unit suite 23 passed. Offline metadata, UI all-target clippy with warnings denied, 28-crate layer validation, wasm checks, both OHOS release targets and native HAP verification passed. Fixed-size 2880 × 1920 API 20 simulator screenshots show a new Untitled-1 [before confirmation](save-title-before.jpeg) and [after saving as renamed-pad.psd](save-title-after.jpeg). The independent io.github.storytold.photocraft.savetitletest bundle leaves the regular application untouched. The real Pad was not deployed or relaunched.
