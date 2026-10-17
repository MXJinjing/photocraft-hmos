import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

// Window-level immersive layout: the page covers the whole window, including the status bar and
// the navigation-indicator strip. The page itself then keeps its controls out of those strips
// through the insets measured in NavigationInset.ets and applied by the native editor.
// This is the documented route (setWindowLayoutFullScreen + getWindowAvoidArea). It is skipped on
// 2-in-1 devices, whose freeform windows and integrated title bar are managed by WindowChrome.
export const IMMERSIVE_KEY: string = 'photocraftImmersive';
// Status-bar height captured before setWindowLayoutFullScreen. Some devices report
// TYPE_SYSTEM.topRect.height as 0 afterwards; NavigationInset falls back to this cached value.
export const STATUS_BAR_PX_KEY: string = 'photocraftStatusBarPx';

export function immersiveWanted(): boolean {
  return deviceInfo.deviceType !== '2in1';
}

function finiteOrZero(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.round(value) : 0;
}

// Best available status-bar / top cutout height in physical pixels.
export function readStatusBarPx(win: window.Window): number {
  try {
    let system = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM).topRect.height;
    let cutout = 0;
    try { cutout = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_CUTOUT).topRect.height; }
    catch (error) { /* Cutout type unavailable on some builds. */ }
    return Math.max(finiteOrZero(system), finiteOrZero(cutout));
  } catch (error) {
    return 0;
  }
}

// Never rejects: on failure the page keeps the old (avoided) layout and no inset is reported.
export function enableImmersiveLayout(win: window.Window): Promise<void> {
  if (!immersiveWanted()) {
    AppStorage.setOrCreate(IMMERSIVE_KEY, false);
    AppStorage.setOrCreate(STATUS_BAR_PX_KEY, 0);
    return Promise.resolve();
  }
  // Capture before fullscreen: avoid-area queries after the call can return a zero top on device.
  let cached = readStatusBarPx(win);
  AppStorage.setOrCreate(STATUS_BAR_PX_KEY, cached);
  console.info(`PhotoCraft status bar cached before immersive: ${cached}px`);
  return win.setWindowLayoutFullScreen(true).then((): void => {
    // Prefer a post-fullscreen reading when the system still publishes one; keep the cache otherwise.
    let after = readStatusBarPx(win);
    if (after > 0) {
      AppStorage.setOrCreate(STATUS_BAR_PX_KEY, after);
      cached = after;
    }
    console.info(`PhotoCraft immersive layout enabled; statusBar=${cached}px`);
    AppStorage.setOrCreate(IMMERSIVE_KEY, true);
  }).catch((error: BusinessError): void => {
    console.error(`PhotoCraft immersive layout unavailable: ${error.code} ${error.message}`);
    AppStorage.setOrCreate(IMMERSIVE_KEY, false);
  });
}
