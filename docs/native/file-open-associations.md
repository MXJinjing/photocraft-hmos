# System file opening

EntryAbility declares `ohos.want.action.viewData` with `scheme: file`, `type`
and `linkFeature: FileOpen`. The previous `utd`/`maxFileSupported` entries
described sharing rather than file-open intent matching.

Standard types cover PSD, JPEG, PNG, TIFF, GIF, BMP, ICO, TGA, OpenEXR,
portable bitmap/graymap/pixmap and camera RAW. Custom UTDs register PCraft,
PSB/PSDT, WebP, HEIF, QOI, HDR, PAM/PFM and supported camera RAW extensions.
Each custom TypeId starts with the actual bundle name
`io.github.storytold.photocraft.hmos.`. No API 26 directory feature is used;
the existing minimum API 20 remains unchanged.

The existing onCreate/onNewWant handlers read the granted URI into the native
editor. Opening does not copy the document into the Downloads application
directory. Saving continues to use the document workflow described in
[document-storage-and-keyboard.md](document-storage-and-keyboard.md).

Validation:

- `python3 scripts/test_file_open.py`: valid declarations plus negative cases
  for missing type/FileOpen, incorrect bundle prefix and missing custom UTD.
- `scripts/dev.sh build`: dual OHOS Rust targets and HAP build; packaged
  capabilities, both native libraries and the native-only assets are verified.
- API 20 simulator, independent workflowtest bundle: an implicit PSD viewData
  request opened the document on cold start. A PNG request with the system
  picker flag showed PhotoCraft in “选择打开方式”; selecting it delivered the
  PNG to the running editor and retained the PSD tab.
- The original app and the real Pad were not reinstalled. The independent
  test bundle was removed after validation. Individual RAW codecs and every
  custom format were not exercised in this association test.

Evidence: [system picker](file-open-picker.jpeg),
[running editor receives another file](file-open-warm.jpeg).

Reference: [OpenHarmony file-processing application integration](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/application-models/file-processing-apps-startup.md).
