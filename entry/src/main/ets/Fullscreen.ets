import native from 'libphotocraft.so';
import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

// Optional window full screen, requested from the egui status-bar button / Window menu item.
// Off by default: nothing here runs until Rust sends a `fullscreen` request after a user click.
// Full screen = setWindowLayoutFullScreen(true) + hidden system bars; the page fills the whole
// window and egui paints edge-to-edge (no inset padding). Leaving restores the avoided layout and
// both bars, and the caller repaints the theme colour on them (SystemBars.ets).

export const FULLSCREEN_KEY: string = 'photocraftFullscreen';

class FullscreenState {
  kind: string = 'fullscreen_state';
  text: string = 'off';
  constructor(on: boolean) { this.text = on ? 'on' : 'off'; }
}

// 2in1 freeform shares the title strip with WindowChrome; immersive layout would fight that, so the
// controls are not offered there (Rust gets `fullscreen: false` in its config).
export function fullscreenAvailable(): boolean {
  return deviceInfo.deviceType !== '2in1';
}

export function fullscreenOn(): boolean {
  return AppStorage.get<boolean>(FULLSCREEN_KEY) === true;
}

// Full screen only makes sense for a window that fills the display; floating and split windows
// keep their system bars and avoid areas.
export function windowAllowsFullscreen(win: window.Window): boolean {
  try {
    let status: window.WindowStatusType = win.getWindowStatus();
    return status === window.WindowStatusType.FULL_SCREEN || status === window.WindowStatusType.MAXIMIZE;
  } catch (error) {
    // Status unavailable on this system version: do not block the user's request.
    return true;
  }
}

export async function enterFullscreen(win: window.Window): Promise<void> {
  await win.setWindowLayoutFullScreen(true);
  await win.setWindowSystemBarEnable([]);
}

export async function exitFullscreen(win: window.Window): Promise<void> {
  await win.setWindowLayoutFullScreen(false);
  await win.setWindowSystemBarEnable(['status', 'navigation']);
}

// Tell Rust the real state so the button icon and menu text follow it (also after system exits).
export function reportFullscreen(on: boolean): void {
  try {
    native.input(JSON.stringify(new FullscreenState(on)));
  } catch (error) {
    console.info('Fullscreen state not delivered before native init');
  }
}

// Apply a request from Rust. Always ends by reporting the resulting state, even on failure, so the
// egui controls never show a state the window is not in. `repaint` recolours the restored bars.
export async function requestFullscreen(win: window.Window, on: boolean, repaint: () => void): Promise<void> {
  let target: boolean = on && fullscreenAvailable() && windowAllowsFullscreen(win);
  if (target === fullscreenOn()) {
    reportFullscreen(target);
    return;
  }
  try {
    if (target) {
      await enterFullscreen(win);
    } else {
      await exitFullscreen(win);
    }
  } catch (error) {
    let failure = error as BusinessError;
    console.info(`PhotoCraft fullscreen ${target ? 'enter' : 'exit'}: ${failure.code} ${failure.message}`);
    reportFullscreen(fullscreenOn());
    return;
  }
  AppStorage.setOrCreate(FULLSCREEN_KEY, target);
  if (!target) { repaint(); }
  reportFullscreen(target);
}

// Keeps the state honest when the system changes the window by itself (floating / split window
// from a system gesture) and re-asserts hidden bars after the app returns to the foreground.
export class FullscreenSync {
  private win: window.Window;
  private repaint: () => void;
  private watching: boolean = false;
  private statusChanged = (status: window.WindowStatusType): void => {
    if (!fullscreenOn()) { return; }
    if (status !== window.WindowStatusType.FULL_SCREEN && status !== window.WindowStatusType.MAXIMIZE) {
      requestFullscreen(this.win, false, this.repaint)
        .catch((error: BusinessError) => { console.info(`PhotoCraft fullscreen sync: ${error.code} ${error.message}`); });
    }
  };

  constructor(win: window.Window, repaint: () => void) { this.win = win; this.repaint = repaint; }

  start(): void {
    if (this.watching || !fullscreenAvailable()) { return; }
    try {
      this.win.on('windowStatusChange', this.statusChanged);
      this.watching = true;
    } catch (error) {
      let failure = error as BusinessError;
      console.info(`Fullscreen status watch unavailable: ${failure.code} ${failure.message}`);
    }
  }

  stop(): void {
    if (!this.watching) { return; }
    this.watching = false;
    try {
      this.win.off('windowStatusChange', this.statusChanged);
    } catch (error) {
      let failure = error as BusinessError;
      console.info(`Fullscreen status unwatch: ${failure.code} ${failure.message}`);
    }
  }

  // Resume: the system may have restored the bars while backgrounded.
  reassert(): void {
    if (!fullscreenOn()) { return; }
    this.win.setWindowSystemBarEnable([]).catch((error: BusinessError) => {
      console.info(`PhotoCraft fullscreen resume: ${error.code} ${error.message}`);
    });
    reportFullscreen(true);
  }
}
