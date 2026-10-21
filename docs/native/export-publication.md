# HarmonyOS export publication

Public DOWNLOAD folders and folders granted by the document picker cannot use
POSIX rename/unlink for the engine's atomic temporary files. Previously, engine
exports (including Image Processor and Lens Correction) could leave a hidden
`.pcsave.tmp` without publishing the final file. Ordinary Save already used
CoreFileKit and did not share that write path.

The OHOS worker now registers a writer in `photocraft_format::atomic`. All
`atomic_write` and `atomic_write_progress` callers therefore share the same
publication path, including batch commands, layer exports, document copies,
presets and action exports. Writes within `filesDir`, and relative writes in the
private working directory, continue to use POSIX atomic replacement.

For public paths the worker waits for an `export_write` reply. ArkTS validates
that the target is beneath the DOWNLOAD root or a folder returned by the output
picker. It stages the encoded bytes in private storage, backs up an existing
file, and publishes through a CoreFileKit file descriptor. Publication errors
propagate to the command; there is no fallback to POSIX rename. A failed restore
retains the private recovery copy and reports its location. Publishing into a
public directory cannot promise atomic replacement across process death; the
private staging/backup protects against reported write failures.

Ordinary Save and Save As retain their existing targets and picker behavior.
Existing temporary files from earlier failures are not deleted automatically.

Verification:

```sh
node scripts/test_local_documents.cjs <typescript-package-path>
cargo test --offline --manifest-path native/rust/Cargo.toml
scripts/dev.sh build
```

Manual PC simulator checks: export PNG/JPEG/TIFF/WebP/PSD/PCraft, overwrite a
completed file, then run Image Processor, Lens Correction and Layers to Files
with the default output directories. Open the completed files and confirm the
new export leaves no `.pcsave.tmp` file. Check a folder chosen in the system
picker as well.
