import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

// Window-level immersive layout: the page covers the whole window, including the status bar and
// the navigation-indicator strip. The page itself then keeps its controls out of those strips
// through the insets measured in NavigationInset.ets and applied by the native editor.
// This is the documented route (setWindowLayoutFullScreen + getWindowAvoidArea). It is skipped on
// 2-in-1 devices, whose freeform windows and integrated title bar are managed by WindowChrome.
export const IMMERSIVE_KEY: string = 'photocraftImmersive';

export function immersiveWanted(): boolean {
  return deviceInfo.deviceType !== '2in1';
}

// Never rejects: on failure the page keeps the old (avoided) layout and no inset is reported.
export function enableImmersiveLayout(win: window.Window): Promise<void> {
  if (!immersiveWanted()) {
    AppStorage.setOrCreate(IMMERSIVE_KEY, false);
    return Promise.resolve();
  }
  return win.setWindowLayoutFullScreen(true).then((): void => {
    console.info('PhotoCraft immersive layout enabled');
    AppStorage.setOrCreate(IMMERSIVE_KEY, true);
  }).catch((error: BusinessError): void => {
    console.error(`PhotoCraft immersive layout unavailable: ${error.code} ${error.message}`);
    AppStorage.setOrCreate(IMMERSIVE_KEY, false);
  });
}
