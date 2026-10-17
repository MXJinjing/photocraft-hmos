import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';

// Page content extends under the navigation indicator (the small bar at the bottom of the screen)
// through `ignoreLayoutSafeArea` + `LayoutPolicy.matchParent`. This watches the window and reports
// the geometry the native editor needs, in physical pixels: the indicator height, the window
// height, and where the editor surface sits after layout. Rust compares them with the real surface
// size, so an inset exists only while the surface really reaches under the indicator (not in
// floating / split windows, not when the keyboard has shrunk the page, and not if the platform
// declined to extend the component).

export class NavigationGeometry {
  // Height of the navigation indicator avoid area; 0 when the window does not touch it.
  indicator: number = 0;
  // Window height.
  windowHeight: number = 0;
  // Top edge of the editor surface in window coordinates.
  contentTop: number = 0;
  // Layout height of the editor surface (includes the bottom extension once ignoreLayoutSafeArea
  // takes effect). 0 until the first onAreaChange.
  contentHeight: number = 0;
  constructor(indicator: number, windowHeight: number, contentTop: number, contentHeight: number = 0) {
    this.indicator = indicator;
    this.windowHeight = windowHeight;
    this.contentTop = contentTop;
    this.contentHeight = contentHeight;
  }
  equals(other: NavigationGeometry | undefined): boolean {
    return other !== undefined && other.indicator === this.indicator &&
      other.windowHeight === this.windowHeight && other.contentTop === this.contentTop &&
      other.contentHeight === this.contentHeight;
  }
}

// Avoid area, window size and component layout reach the app in separate transactions;
// read once more after they settle.
const SETTLE_MS: number = 250;

function finiteOrZero(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.round(value) : 0;
}

export class NavigationIndicatorInset {
  private win: window.Window | undefined = undefined;
  private contentTop: number = 0;
  private contentHeight: number = 0;
  private sent: NavigationGeometry | undefined = undefined;
  private settle: number = -1;
  private publish: (geometry: NavigationGeometry) => void;
  private avoidChanged = (data: window.AvoidAreaOptions): void => {
    if (data.type === window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR) { this.refresh(); }
  };
  private sizeChanged = (_size: window.Size): void => { this.refresh(); };

  constructor(publish: (geometry: NavigationGeometry) => void) { this.publish = publish; }

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

  // Top edge and layout height of the editor surface, physical pixels in window coordinates.
  setContentBounds(topPx: number, heightPx: number): void {
    this.contentTop = finiteOrZero(topPx);
    this.contentHeight = finiteOrZero(heightPx);
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

  private read(win: window.Window): NavigationGeometry | undefined {
    try {
      let indicator: number = finiteOrZero(
        win.getWindowAvoidArea(window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR).bottomRect.height);
      if (indicator === 0) {
        return new NavigationGeometry(0, 0, 0, 0);
      }
      let height: number = finiteOrZero(win.getWindowProperties().windowRect.height);
      return new NavigationGeometry(indicator, height, this.contentTop, this.contentHeight);
    } catch (error) {
      // A transient failure must neither invent nor erase an inset: keep what native already has.
      return undefined;
    }
  }

  private update(): void {
    let win = this.win;
    if (win === undefined) { return; }
    let geometry = this.read(win);
    if (geometry === undefined || geometry.equals(this.sent)) { return; }
    this.sent = geometry;
    this.publish(geometry);
  }
}
