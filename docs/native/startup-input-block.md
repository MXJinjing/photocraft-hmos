# PC 2in1 emulator APP_INPUT_BLOCK at startup

## Symptom

On the PC 2in1 emulator (HarmonyOS 7 / API 26 class), PhotoCraft 0.9.0.1 can open,
create the XComponent surface, then freeze. After about eight seconds the system
reports `APP_INPUT_BLOCK` / ANR and kills the process. Hilog typically shows:

1. `DocumentViewPicker` DOWNLOAD (`downloadAuth`) finishing
2. Rust host init and `Surface created …×…`
3. `HarmonyOS stylus doubleTap/squeeze listen` errors (`undefined is not callable` / missing `.on`)
4. No further application logs before the freeze dump

CPU usage stays low, so the ArkTS thread is waiting rather than busy-looping.

## Cause

After `native.init`, `HarmonyStylusAdapter.start()` immediately scanned InputKit
with **synchronous** `getKeyboardTypeSync` / `getDeviceInfoSync` on the ArkTS
thread, and could call `requestPermissionsFromUser(ACCESS_BLUETOOTH)` as soon as
any handwriting-pen device appeared. On the PC 2in1 emulator Penkit listeners
are unavailable and those Binder/permission paths can stall input marking long
enough for the 8s watchdog.

GPU `pollster::block_on` during attach runs on the **render worker**, not the
input thread; it can still stall drawing on bad GLES, but it is not the direct
`APP_INPUT_BLOCK` trigger. Extra `stage` notifications around adapter/device
creation make the two stalls easier to tell apart in hilog.

## Fix (`fix/startup-input-block`)

- Probe devices with async `getKeyboardType` / `getDeviceInfo` only
- Defer the first scan by 1.5s and poll every 5s instead of every 1s
- Do not auto-prompt for Bluetooth during startup (battery enrichment stays off
  until a future pen-settings path grants access)
- Keep GPU init on the worker; add stage logs around adapter/device requests

No PR until device/emulator acceptance.
