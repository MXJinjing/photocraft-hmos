# Persisted Picker URIs for Open Recent

2026-10-19 (Asia/Shanghai)

## Goal

Home › Recent / File › Open Recent must reopen documents that were opened or saved
through `DocumentViewPicker` after the app restarts. Session-only `save-default/…`
tokens cannot do that ([recent-ephemeral-save-tokens.md](recent-ephemeral-save-tokens.md)).

## Approach

1. Declare `ohos.permission.FILE_ACCESS_PERSIST` (normal, `system_grant`, API 12+) in
   `entry/src/main/module.json5`.
2. After a successful Picker **open** or **save**, when
   `canIUse('SystemCapability.FileManagement.AppFileService.FolderAuthorization')` is
   true, call `fileShare.persistPermission` with `READ_MODE | WRITE_MODE`.
3. Prefer a durable key for native Recent / Save:
   - App download-directory path when `LocalDocuments.owns` (unchanged).
   - Else the system `file://…` URI when persist succeeds.
   - Else the legacy `save-default/` / `save-as/` token (still scrubbed from Recent).
4. Opening a `file://` Recent entry goes through host bridge `open_uri`:
   `activatePermission` → `fileIo.open` → `replyFile`. Native `services.read_uri`
   supplies those bytes to `open_path` (never `std::fs` on a URI).
5. Writing back to a persisted URI activates first, then opens with CoreFileKit.
   Ordinary Save may reuse a persisted PSD/PSB URI as the source without another picker.

## Limitations

- Persist only works for URIs the Picker already granted; uninstall clears grants.
- Activation is required after every app or device restart (done on demand per open/save).
- Devices without `FolderAuthorization` keep the previous ephemeral-token behavior.
- Not fully exercised on this Linux box (no HarmonyOS device / simulator here). Validate
  on Pad: Save outside download dir → kill app → Home › Recent reopen → Save again.

## Files

- `entry/src/main/ets/persistPermission.ets` — capability check, persist, activate (+ retry)
- `entry/src/main/ets/pages/Index.ets` — open / save / write / `open_uri`
- `entry/src/main/ets/savePicker.ets` — durable keys in `SaveTargets`
- `native/rust/src/documents.rs` — accept `file://` ordinary-save targets
- `native/rust/src/lib.rs` — `services.read_uri` → bridge `open_uri`
- `upstream/.../file_open.rs` — `is_file_uri` + `open_path` host read

## Verification

- `cargo test -p photocraft-ui-egui --lib ephemeral_save_tokens file_uri_open`
- `cargo test -p photocraft-native --lib documents`
- `node scripts/test_persist_permission.cjs` / `node scripts/test_save_picker.cjs`
