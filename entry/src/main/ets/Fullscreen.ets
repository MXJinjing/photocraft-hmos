import native from 'libphotocraft.so';
import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

// Optional window full screen, requested from the egui status-bar button / Window menu item.
// Off by default: nothing here runs until Rust sends a `fullscreen` request after a user click.
// Full screen = setWindowLayoutFullScreen(true) + hidden system bars; the page fills the whole
// window and egui paints edge-to-edge (no inset padding). Leaving restores the avoided layout and
// both bars, and the caller repaints the theme colour on them (SystemBars.ets).
//
// Desire (AppStorage) vs applied immersion: tablet → free multi-window (FLOATING / MAXIMIZE /
// SPLIT) pauses immersion — bars and avoided layout come back — but keeps the user's desire.
// Returning to FULL_SCREEN (平板模式) re-applies hide-bars + layout full screen. Without that
// restore, the window could fill the display while status / navigation bars stayed visible.

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

// Immersive full screen only belongs to tablet/phone FULL_SCREEN. Free multi-window maximized
// reports MAXIMIZE and still shows the system caption / bars — treating it like FULL_SCREEN left
// desire on while the system re-showed bars, so returning to tablet filled the screen without
// hiding them again.
export function windowAllowsFullscreen(win: window.Window): boolean {
  try {
    return win.getWindowStatus() === window.WindowStatusType.FULL_SCREEN;
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

// Tell Rust the real applied state so the button icon and menu text follow the window
// (also after system exits / freeform pause).
export function reportFullscreen(on: boolean): void {
  try {
    native.input(JSON.stringify(new FullscreenState(on)));
  } catch (error) {
    console.info('Fullscreen state not delivered before native init');
  }
}

// Apply a request from Rust. Always ends by reporting the resulting *applied* state, even on
// failure, so the egui controls never show a state the window is not in. `repaint` recolours the
// restored bars. Desire is stored in AppStorage; freeform refuses a fresh enter without clearing
// an already-paused desire.
export async function requestFullscreen(win: window.Window, on: boolean, repaint: () => void): Promise<void> {
  if (!on) {
    if (!fullscreenOn()) {
      reportFullscreen(false);
      return;
    }
    try {
      await exitFullscreen(win);
    } catch (error) {
      let failure = error as BusinessError;
      console.info(`PhotoCraft fullscreen exit: ${failure.code} ${failure.message}`);
      reportFullscreen(false);
      return;
    }
    AppStorage.setOrCreate(FULLSCREEN_KEY, false);
    repaint();
    reportFullscreen(false);
    return;
  }

  if (!fullscreenAvailable() || !windowAllowsFullscreen(win)) {
    // Fresh enter refused in freeform/split/2in1. A paused desire stays so tablet restore works.
    reportFullscreen(false);
    return;
  }
  try {
    await enterFullscreen(win);
  } catch (error) {
    let failure = error as BusinessError;
    console.info(`PhotoCraft fullscreen enter: ${failure.code} ${failure.message}`);
    AppStorage.setOrCreate(FULLSCREEN_KEY, false);
    reportFullscreen(false);
    return;
  }
  AppStorage.setOrCreate(FULLSCREEN_KEY, true);
  reportFullscreen(true);
}

// Keeps desire honest across window-mode changes: pause immersion when leaving FULL_SCREEN
// (free multi-window / split), re-apply when returning to tablet FULL_SCREEN; reassert on
// foreground if desire is still on.
export class FullscreenSync {
  private win: window.Window;
  private repaint: () => void;
  private watching: boolean = false;
  private statusChanged = (status: window.WindowStatusType): void => {
    if (!fullscreenOn()) { return; }
    if (status === window.WindowStatusType.FULL_SCREEN) {
      enterFullscreen(this.win)
        .then(() => { reportFullscreen(true); })
        .catch((error: BusinessError) => {
          console.info(`PhotoCraft fullscreen restore: ${error.code} ${error.message}`);
        });
      return;
    }
    // Pause: show bars again for freeform chrome, keep AppStorage desire for later restore.
    exitFullscreen(this.win)
      .then(() => { this.repaint(); reportFullscreen(false); })
      .catch((error: BusinessError) => {
        console.info(`PhotoCraft fullscreen pause: ${error.code} ${error.message}`);
      });
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
    if (!windowAllowsFullscreen(this.win)) {
      reportFullscreen(false);
      return;
    }
    enterFullscreen(this.win).catch((error: BusinessError) => {
      console.info(`PhotoCraft fullscreen resume: ${error.code} ${error.message}`);
    });
    reportFullscreen(true);
  }
}
