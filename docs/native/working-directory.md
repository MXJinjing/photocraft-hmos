# Native working directory

The HarmonyOS process initially inherits `/` as its current directory. Native
editor startup now creates `UIAbilityContext.filesDir/work` and selects it with
`std::env::set_current_dir` before constructing the editor. Creation or directory
selection failures are reported through the host error channel and stop editor
startup. There is no silent fallback to `/`.

This is a process-wide working directory, selected once by the native worker.
Relative paths in engine scripts and batch commands resolve here. Explicit
absolute paths still refer to their specified locations. The preferences path,
ordinary PSD save destination in the application Download directory, and Save As
picker remain independent of the working directory. This change does not change
`std::env::temp_dir()` or add permissions for external files.

Validation:

- Native library tests: 22 passed. An isolated subprocess exercises directory
  creation, repeated initialization, relative read/write/rename/delete, and
  rejection of relative roots or a file in place of a directory.
- Both OHOS Rust architectures and the native HAP build passed. Packaged native
  assets passed verification. ARM64 OHOS production library clippy passed with
  warnings denied. Host all-target clippy still reports host-only dead code and
  test unwrap lints; it is not a passing check.
- API 20 simulator, independent workflowtest bundle: startup reported
  `/data/storage/el2/base/haps/entry/files/work`. A three-step action script
  created a 16×16 document, exported `work-probe.psd` through `file.saveACopy`,
  then read that relative path with `file.scripts.loadFilesIntoStack`. All three
  commands returned success; CoreFileKit confirmed the PSD was 862 bytes in
  `files/work`.
- The independent test bundle was removed. The original application and real
  Pad were not reinstalled.

Local evidence: `work/sandbox-work-device.log`, `work/sandbox-work-build.log`,
`work/sandbox-work-native-tests.log`, `work/sandbox-work-ohos-clippy.log`.
