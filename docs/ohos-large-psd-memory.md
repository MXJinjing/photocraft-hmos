# HarmonyOS large PSD admission and scratch storage

## Root cause and wiring

- An unreadable memory cgroup is not a measurement of zero free RAM. The OHOS-only
  probe falls back to MemAvailable, then sysinfo; no measurement still fails closed.
- Scratch preferences default to an enabled `(system temp)` disk. On OHOS this must
  resolve to the app sandbox, not Rust's OS-wide temporary directory. ArkTS now
  passes `context.cacheDir` in `NativeConfig.cache`; the native worker creates
  `cacheDir/photocraft-scratch` and installs it before session/preferences setup.
  Legacy host configs without cacheDir use `filesDir/cache/photocraft-scratch`.
- `engine::scratch::configure` uses this override only on OHOS and only for the
  placeholder. Custom paths, disabled disks and explicit size limits are preserved.
  Resetting preferences still resolves to the private cache. A missing override
  disables the placeholder disk instead of falling back to a global directory.
- Raster's existing scratch manager creates the backing file, queries statvfs,
  reserves max(1 GiB, 5% of the volume), and subtracts allocator overhead. On OHOS,
  each newly opened backing file first performs a bounded 4 KiB positioned-write,
  read and CRC round trip on the query thread; failure pauses it before admission
  can promise disk capacity. A paused,
  full, disabled or unusable disk earns no credit. An unknown volume without an
  explicit size cap earns no credit either. Explicit size caps retain upstream's
  existing behavior for unmeasured volumes; they are not inferred from RAM.
- The old `pc_init` diagnostic ran before the engine configured any disk: its zero
  credit alone did NOT prove that spilling was disabled after initialization.
  OHOS now logs after preference loading, with directory, usable scratch capacity
  (after reserve, not raw disk-free bytes), resident budget, pause/error state and
  written-byte count. Startup normally has zero written bytes until tiles spill.

## No new blanket 2 GiB override

`DEFAULT_PSD_IMPORT_PIXEL_BYTES` remains unchanged. It is a historical explicit
allowance for `psd_budget::check` callers/tests, not a cap in the production open
path. `import_psd` already calls `evaluate_psd_admission` before and after acquiring
its decode gate, with the measured RAM snapshot and the estimate's spillable tile
share. Default `ImportOptions.psd_max_pixel_bytes` is zero (no caller cap).
Adding another OHOS allowance would weaken the existing gate, not fix a real cap.

The RAM hard limit is generally 75% of current available RAM, not installed RAM.
Only retained tiles can earn measured scratch credit; merged planes, decode and
conversion workspace must fit RAM. Approval cannot bypass the hard limit. A
12 GiB device does not guarantee every 2 GiB compressed file will fit: dimensions,
bit depth, layers, compression, current RAM, disk space and other apps all matter.

## Remaining safety limits (intentionally retained)

- PSD codec `compression::MAX_DECODED_BYTES`: 8 GiB on 64-bit, 2 GiB on 32-bit,
  per decoded channel layout/output path rather than aggregate import/file size.
- `psd_import::MAX_UNFILTERED_BYTES`: 1 GiB retained unfiltered smart-filter cache.
  Beyond it the importer keeps stored pixels; it does not reject the whole PSD.
- Pattern padding: 512 MiB aggregate zero padding; decoded patterns also share the
  codec's 8 GiB decoded budget. These prevent malicious sparse allocations.
- Header dimensions: at most 300,000 per side; 56 channels; group/descriptor nesting
  and parser/addressability checks remain in effect.
- PSD export's 2 GiB file limit remains; larger exports need PSB. It is not an open
  file-size limit, and it is not a decoded-memory estimate.
- ArkTS picker/drop opens an authorized file descriptor, without a 2 GiB JS buffer.
  Folder picking uses filesystem copyFile without an app-imposed size ceiling.
  Native `file_open::read_sized` reads 64 KiB chunks into one reserved Vec, bounded
  by addressability/allocation, not 2 GiB. The encoded input still occupies RAM,
  and parser-owned buffers may add memory. This is not fully streaming decoding.

## Regression coverage

- A tiny real PSD fixture with synthetic charges exceeds 6 GiB aggregate peak:
  6 GiB available RAM plus measured scratch admits it without a legacy 2 GiB cap;
  no scratch, tiny RAM, insufficient workspace RAM and a lower explicit caller
  limit still refuse it. No multi-gigabyte allocation is needed by the test.
- An isolated raster integration test installs the private cache override, forces
  real tile eviction, verifies file length/written bytes and lossless reload,
  then verifies disabled, size-capped and unusable storage cannot over-promise.
- Native path tests and the ArkTS config-serialization test cover cacheDir and the
  old-host fallback. A probe regression checks all scratch diagnostic fields.

Real-device acceptance with the user's large PSD/PSB is still required; emulator
startup and synthetic admission tests are not proof that that file opens.
