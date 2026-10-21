# Recent files must not list host save tokens

2026-10-18 (Asia/Shanghai)

## Bug

On HarmonyOS, ordinary Save and Save As go through the system document picker. The host
returns a session-scoped token (`save-default/<id>/<name>` or `save-as/<id>/<name>`) that
maps to a granted URI in ArkTS `SaveTargets`. After a successful write, upstream recorded
that token in Open Recent / the Home screen recent strip.

After the app restarts (or the page drops the URI map), tapping such a Recent entry calls
`open_path` on the token string. It is not a filesystem path, so open fails. Users see
entries like `save-default/…/foo.psd` that can never reopen.

## Fix

- `file_open::is_ephemeral_save_token` recognizes the two token prefixes.
- `push_recent` ignores those paths (real paths under the app download directory still list).
- `sync_recent` scrubs any leftover tokens from preferences so older installs heal on launch.
- `open_path` refuses a token, removes it from Recent, and reports that the save target is gone.

Real document-directory paths (`Documents.owns`) are unchanged and still appear in Recent.

## Verification

- `cargo test -p photocraft-ui-egui ephemeral_save_tokens_are_not_listed_in_recent ephemeral_save_token_recent_is_dropped_on_open`
- Broader open-recent tests still pass.
