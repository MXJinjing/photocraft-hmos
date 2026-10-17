import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';

// With the window in immersive layout (Immersive.ets) the page covers the status bar and the
// navigation-indicator strip. This watches the window and decides, in physical pixels, how much of
// the top and bottom of the editor surface those system areas really cover:
//
//   system area height  = avoid area from getWindowAvoidArea (status bar on top; navigation
//                         indicator / navigation bar on the bottom, taking the larger of the
//                         NAVIGATION_INDICATOR and SYSTEM reports)
//   already avoided     = distance between the surface and the window edge, measured from the
//                         laid-out XComponent (this is what the on-screen keyboard shrinks under
//                         KeyboardAvoidMode.RESIZE, and what a non-immersive layout leaves free)
//   inset               = max(0, system area height - already avoided)
//
// The native editor receives only the final insets. Nothing is guessed on the Rust side.

export class SafeInsets {
  top: number = 0;
  bottom: number = 0;
  constructor(top: number, bottom: number) { this.top = top; this.bottom = bottom; }
  equals(other: SafeInsets | undefined): boolean {
    return other !== undefined && other.top === this.top && other.bottom === this.bottom;
  }
}

export function insetsFor(systemTop: number, systemBottom: number, windowHeight: number,
  contentTop: number, contentHeight: number): SafeInsets {
  let top: number = systemTop > 0 ? Math.max(0, systemTop - Math.max(0, contentTop)) : 0;
  let bottom: number = 0;
  if (systemBottom > 0) {
    // Unknown window height: the surface cannot be compared, so assume it reaches the edge.
    let gap: number = windowHeight > 0 ? Math.max(0, windowHeight - (contentTop + contentHeight)) : 0;
    bottom = Math.max(0, systemBottom - gap);
  }
  return new SafeInsets(Math.round(top), Math.round(bottom));
}

// Avoid area, window size and component layout reach the app in separate transactions;
// read once more after they settle.
const SETTLE_MS: number = 250;

function finiteOrZero(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.round(value) : 0;
}

export class NavigationIndicatorInset {
  private win: window.Window | undefined = undefined;
  private immersive: boolean = false;
  private contentTop: number = 0;
  private contentHeight: number = 0;
  private laidOut: boolean = false;
  private sent: SafeInsets | undefined = undefined;
  private settle: number = -1;
  private publish: (insets: SafeInsets) => void;
  private report: (text: string) => void;
  private avoidChanged = (_data: window.AvoidAreaOptions): void => { this.refresh(); };
  private sizeChanged = (_size: window.Size): void => { this.refresh(); };

  // `report` receives one diagnostic line per change (hilog and the optional on-screen overlay).
  constructor(publish: (insets: SafeInsets) => void, report: (text: string) => void) {
    this.publish = publish;
    this.report = report;
  }

  start(win: window.Window): void {
    this.stop();
    this.win = win;
    try {
      win.on('avoidAreaChange', this.avoidChanged);
      win.on('windowSizeChange', this.sizeChanged);
    } catch (error) {
      let failure = error as BusinessError;
      console.info(`Navigation indicator listeners unavailable: ${failure.code} ${failure.message}`);
    }
    this.update();
  }

  // Whether the window runs in immersive layout; without it the page is already inside the safe
  // area and no inset exists.
  setImmersive(on: boolean): void {
    this.immersive = on;
    this.update();
  }

  // Top edge and layout height of the editor surface, physical pixels in window coordinates.
  setContentBounds(topPx: number, heightPx: number): void {
    this.contentTop = finiteOrZero(topPx);
    this.contentHeight = finiteOrZero(heightPx);
    this.laidOut = true;
    this.update();
  }

  // The native side lost its value (new surface or worker): publish the current one again.
  resend(): void {
    this.sent = undefined;
    this.update();
  }

  stop(): void {
    if (this.settle >= 0) { clearTimeout(this.settle); this.settle = -1; }
    let win = this.win;
    this.win = undefined;
    this.sent = undefined;
    this.laidOut = false;
    if (win === undefined) { return; }
    try {
      win.off('avoidAreaChange', this.avoidChanged);
      win.off('windowSizeChange', this.sizeChanged);
    } catch (error) {
      console.info('Navigation indicator listeners already released');
    }
  }

  private refresh(): void {
    this.update();
    if (this.settle >= 0) { clearTimeout(this.settle); }
    this.settle = setTimeout((): void => { this.settle = -1; this.update(); }, SETTLE_MS);
  }

  private read(win: window.Window): SafeInsets | undefined {
    if (!this.immersive) {
      this.report(`immersive=false insets=0/0`);
      return new SafeInsets(0, 0);
    }
    if (!this.laidOut) { return undefined; }
    try {
      let system = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      let nav = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
      let systemTop: number = finiteOrZero(system.topRect.height);
      let systemBottom: number = Math.max(finiteOrZero(nav.bottomRect.height), finiteOrZero(system.bottomRect.height));
      let windowHeight: number = 0;
      try { windowHeight = finiteOrZero(win.getWindowProperties().windowRect.height); }
      catch (error) { console.info('Window height unavailable'); }
      let insets = insetsFor(systemTop, systemBottom, windowHeight, this.contentTop, this.contentHeight);
      this.report(`immersive=true avoid(sysTop=${systemTop} sysBottom=${finiteOrZero(system.bottomRect.height)} ` +
        `navBottom=${finiteOrZero(nav.bottomRect.height)}) window=${windowHeight} ` +
        `content(top=${this.contentTop} h=${this.contentHeight}) insets=${insets.top}/${insets.bottom}`);
      return insets;
    } catch (error) {
      // A transient failure must neither invent nor erase an inset: keep what native already has.
      return undefined;
    }
  }

  private update(): void {
    let win = this.win;
    if (win === undefined) { return; }
    let insets = this.read(win);
    if (insets === undefined || insets.equals(this.sent)) { return; }
    this.sent = insets;
    this.publish(insets);
  }
}
