import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

// Default immersive fullscreen for phone/tablet: the page fills the whole window and the
// status / navigation system bars are hidden. Content (XComponent / egui) paints edge-to-edge;
// there is no safe-area inset padding on the Rust side.
//
// 2in1 freeform shares the title strip with WindowChrome; immersive fullscreen would fight that,
// so this path is skipped there and the system bars stay under WindowChrome's control.

export const IMMERSIVE_KEY: string = 'photocraftImmersive';

export function immersiveWanted(): boolean {
  return deviceInfo.deviceType !== '2in1';
}

// setWindowLayoutFullScreen(true) then hide status + navigation bars.
// Never rejects: on failure the page keeps the ordinary avoided layout.
export function enableImmersiveFullscreen(win: window.Window): Promise<void> {
  if (!immersiveWanted()) {
    AppStorage.setOrCreate(IMMERSIVE_KEY, false);
    console.info('PhotoCraft immersive fullscreen skipped on 2in1 (WindowChrome owns freeform)');
    return Promise.resolve();
  }
  return win.setWindowLayoutFullScreen(true).then((): Promise<void> => {
    return win.setWindowSystemBarEnable([]).then((): void => {
      AppStorage.setOrCreate(IMMERSIVE_KEY, true);
      console.info('PhotoCraft immersive fullscreen enabled; system bars hidden');
    });
  }).catch((error: BusinessError): void => {
    console.error(`PhotoCraft immersive fullscreen unavailable: ${error.code} ${error.message}`);
    AppStorage.setOrCreate(IMMERSIVE_KEY, false);
  });
}

export function logImmersiveError(action: string, error: BusinessError): void {
  console.info(`PhotoCraft immersive ${action}: ${error.code} ${error.message}`);
}
