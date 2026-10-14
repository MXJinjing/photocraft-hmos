# Large file opening

The HarmonyOS host no longer imposes a 256 MiB input-file limit. Both the
system file picker, external viewData/stream Wants and drag-and-drop file
records pass an authorized file descriptor to the native adapter instead of
allocating a whole-file ArkTS ArrayBuffer.

Rust duplicates the descriptor before NAPI returns. ArkTS closes its own File
after handoff; the external inbox retains only authorized handles while waiting
for cold-start initialization. A background Rust reader reads the duplicate
into one input Vec, then either completes the waiting picker request or queues
the document for import. Reading cannot run on the render worker because that
worker may be waiting for the picker reply. Native PhotoCraft jobs are enabled
on HarmonyOS, so decoding then continues on an open-job worker with a progress
tab while the editor keeps presenting frames.

The reader uses fallible allocation, handles short/interrupted reads, rejects
empty/truncated files, and reports read/allocation failures. Legacy byte-buffer
open/reply entry points also use fallible copying, with only Rust's intrinsic
slice-addressability check. Invalid replies complete with an error rather than
leaving a picker waiter blocked.

This removes an arbitrary file-size ceiling; it does not provide streaming
decoding or unlimited memory. The engine still imports a complete byte slice,
and decoded pixels, layers and render caches use additional memory. Decoder
validation remains intact. An allocator error can be reported, but an OS memory
kill or allocations inside a decoder cannot be made recoverable by this reader.

## Verification (2026-10-12)

- `node scripts/test_large_file_open.cjs <typescript module>` exercises the
  actual ArkTS inbox and picker methods: descriptor handoff without stat/read
  or whole-file buffers, cold/warm multi-file Wants, owned source names,
  cancellation, native errors and handle cleanup.
- `cargo test --offline --manifest-path native/rust/Cargo.toml`: 61 unit tests
  passed. New tests cover a 300 MiB sparse BMP, import after closing the original
  descriptor, short/interrupted reads, empty/truncated inputs, capacity failure,
  invalid replies and asynchronous picker completion. Local testing used
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` to reduce cache size.
- Existing file-association, LocalDocuments and Save/Save As picker tests passed.
- `cargo metadata --offline` from upstream and `scripts/dev.sh build` passed;
  the ARM64 OHOS archive and native HAP were rebuilt and verified.
- OHOS production-library clippy was attempted with `-D warnings`. It is blocked
  by existing `appearance.rs:27` (`let_and_return`) and `input.rs:173`, `:420`,
  `:448` (`unwrap_used` and two `collapsible_if` diagnostics). None is in the
  changed file-opening code.

The API 20 tablet emulator (4 GiB RAM, 2880 × 1920) ran an independent
`io.github.storytold.photocraft.largefiletest` bundle. Fixtures were created
inside that app's sandbox and its system-authorized Downloads directory:

| Entry | Fixture | Result |
| --- | --- | --- |
| Cold-start viewData Want | 314,572,800-byte BMP | Opened and rendered |
| Warm viewData Want | 300,000,040-byte PSD, RGB/8, 10000 × 10000 | Opened and rendered; BMP tab retained |
| Actual Ctrl+O system picker | 314,572,800-byte BMP in Downloads | Opened and rendered; existing tabs retained |
| Warm viewData Want | Empty BMP | Visible error; dismissing it retained all existing documents |

The BMP has 2 × 2 real pixels and a sparse trailing region, so it tests file
transport rather than a complex document. The PSD has a real uncompressed
100-megapixel composite and no layer records. These tests do not establish
performance or fidelity for arbitrary multilayer projects. No real Pad was
deployed to, and the original simulator app was not reinstalled.
The independent simulator test bundle was uninstalled after validation.

Fixed-size evidence: [initial editor](large-file-open-before.png),
[100-megapixel PSD](large-file-open-psd.png),
[large BMP opened through the system picker](large-file-open-picker.png).

### User-supplied 32-bit PSB

`large_file_32bit.psb` was tested on the macOS host with a temporary probe using
the production native `file_open` module and `photocraft_io::import`. The probe
duplicated the file descriptor, closed the original, then read and imported the
complete file. The source file was not modified.

| Measurement | Result |
| --- | --- |
| Input size | 3,919,984,376 bytes (3.65 GiB) |
| Format | PSB version 2, RGB, 32 bits per channel |
| Imported document | 8000 × 4500, `F32`, `Rgb` |
| Imported layers | 4 top-level entries, 80 total entries including groups |
| Descriptor read | 0.890 seconds |
| Import | 54.658 seconds |
| Read + import | 55.548 seconds |
| Import warnings | None |
| Maximum resident set size (`/usr/bin/time -l`) | 15,082,979,328 bytes (14.05 GiB) |
| Peak memory footprint (`/usr/bin/time -l`) | 35,998,411,080 bytes (33.53 GiB) |

This confirms that the updated reader accepts the complete file and the current
parser imports its 32-bit layers. It does not verify canvas rendering, visual
fidelity, editing or saving this document on HarmonyOS. Timing and memory are
host measurements, not Pad measurements.

The existing 4 GiB simulator had only about 2.9 GiB of free data storage, too
little for the input file. A separate 16 GiB RAM / 16 GiB storage simulator was
created, but the emulator refused to start: its disk-space check required
38.40 GB free while the host had about 7 GB free. The temporary simulator was
deleted afterward. No real Pad was used for this PSB test.

Local probe and raw logs are ignored under `work/psb-file-probe`,
`work/psb-host-test.log` and `work/psb-host-metrics.log`.

## Branch verification (2026-10-13)

The large-file integration was restored into an independent feature worktree
based on `main` (`06c35363`), preserving its existing branding and stylus changes.
The native descriptor reader, FFI/NAPI integration, ArkTS handoff and regression
tests are committed together. Simulator fixture injection is excluded.

ArkTS large-file, LocalDocuments, Save/Save As picker tests and the four
file-association tests passed. Upstream Cargo metadata resolved offline and
`git diff --check` passed. The complete native Rust suite had 60 passing tests
and two failing brush-outline tests. All new file-opening tests passed. Both
failures were reproduced separately using the unchanged `main` source:

- `input::tests::fingers_hide_the_rendered_brush_outline_but_pen_and_mouse_keep_it`
- `input::tests::rendered_brush_outline_hides_on_lift_or_leave_and_returns_on_next_hover_or_mouse`

Local logs are in the original checkout's ignored `work/large-file-branch-tests.log`
and `work/large-file-main-baseline-tests.log`. The simulator/HAP and PSB results
above remain the measurements from the previous verification session.
