import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

// 2in1 freeform shares the title strip with WindowChrome; immersive fullscreen would fight that.
export function fullscreenToggleAvailable(): boolean {
  return deviceInfo.deviceType !== '2in1';
}

// Enter HarmonyOS immersive fullscreen and hide status + navigation system bars.
export async function enterFullscreen(win: window.Window): Promise<void> {
  await win.setWindowLayoutFullScreen(true);
  await win.setWindowSystemBarEnable([]);
}

// Leave immersive layout and show the system bars again.
export async function exitFullscreen(win: window.Window): Promise<void> {
  await win.setWindowLayoutFullScreen(false);
  await win.setWindowSystemBarEnable(['status', 'navigation']);
}

export async function applyFullscreen(win: window.Window, on: boolean): Promise<void> {
  if (on) {
    await enterFullscreen(win);
  } else {
    await exitFullscreen(win);
  }
}

export function logFullscreenError(action: string, error: BusinessError): void {
  console.info(`PhotoCraft fullscreen ${action}: ${error.code} ${error.message}`);
}
