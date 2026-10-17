import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { STATUS_BAR_PX_KEY, readStatusBarPx } from './Immersive';

// With the window in immersive layout (Immersive.ets) the page covers the status bar and the
// navigation-indicator strip. This watches the window and decides, in physical pixels, how much of
// the top and bottom of the editor surface those system areas really cover.
//
// Top (status bar): always the full system-area height while immersive. Never subtract the
// laid-out content top — a stale or misreported top used to cancel the inset while the surface
// still painted underneath the status bar (see device screenshot with overlapping menu icons).
// Bottom (navigation indicator): system area minus the gap the layout already leaves at the
// window bottom (keyboard RESIZE shrinks the surface and drives this to 0).
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
  // Immersive surface covers the status bar: use the full height. contentTop is ignored on purpose.
  let top: number = systemTop > 0 ? systemTop : 0;
  let bottom: number = 0;
  if (systemBottom > 0) {
    // Unknown window height: the surface cannot be compared, so assume it reaches the edge.
    let gap: number = windowHeight > 0 ? Math.max(0, windowHeight - (contentTop + contentHeight)) : 0;
    bottom = Math.max(0, systemBottom - gap);
  }
  return new SafeInsets(Math.round(top), Math.round(bottom));
}

// Avoid area, window size and component layout reach the app in separate transactions, and the
// first avoid-area query can run before the system has published the bars. Read again after they
// settle (same idea as the title-bar geometry retries).
const SETTLE_MS: number[] = [250, 1000, 2500];

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
  private settles: number[] = [];
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
    this.refresh();
  }

  // Whether the window runs in immersive layout; without it the page is already inside the safe
  // area and no inset exists.
  setImmersive(on: boolean): void {
    this.immersive = on;
    this.update();
  }

  // Top edge and layout height of the editor surface, physical pixels in window coordinates.
  // Only the bottom gap (windowHeight - top - height) is used; the top edge never cancels the
  // status-bar inset.
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

  // The laid-out box is kept across stop()/start(): the page may lay out before the window query
  // resolves, and no further area event would follow to replace a wiped value.
  stop(): void {
    this.cancelSettles();
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
    this.cancelSettles();
    for (let delay of SETTLE_MS) {
      this.settles.push(setTimeout((): void => { this.update(); }, delay));
    }
  }

  private cancelSettles(): void {
    for (let timer of this.settles) { clearTimeout(timer); }
    this.settles = [];
  }

  private systemTopPx(win: window.Window): number {
    // Live avoid areas first; fall back to the pre-fullscreen cache when the system reports 0.
    let live: number = readStatusBarPx(win);
    if (live > 0) { return live; }
    let cached = AppStorage.get<number>(STATUS_BAR_PX_KEY);
    return finiteOrZero(cached ?? 0);
  }

  private read(win: window.Window): SafeInsets | undefined {
    if (!this.immersive) {
      this.report(`immersive=false insets=0/0`);
      return new SafeInsets(0, 0);
    }
    try {
      let system = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      let nav = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
      let systemTop: number = this.systemTopPx(win);
      let systemBottom: number = Math.max(finiteOrZero(nav.bottomRect.height), finiteOrZero(system.bottomRect.height));
      let windowHeight: number = 0;
      try { windowHeight = finiteOrZero(win.getWindowProperties().windowRect.height); }
      catch (error) { console.info('Window height unavailable'); }
      // Before the first layout report the immersive page is assumed to fill the window, so both
      // insets exist from the very first frame instead of waiting for onAreaChange.
      let insets = this.laidOut ?
        insetsFor(systemTop, systemBottom, windowHeight, this.contentTop, this.contentHeight) :
        new SafeInsets(systemTop, systemBottom);
      this.report(`immersive=true avoid(sysTopLive=${finiteOrZero(system.topRect.height)} ` +
        `sysTopUsed=${systemTop} sysBottom=${finiteOrZero(system.bottomRect.height)} ` +
        `navBottom=${finiteOrZero(nav.bottomRect.height)}) window=${windowHeight} ` +
        `content(laidOut=${this.laidOut} top=${this.contentTop} h=${this.contentHeight}) ` +
        `insets=${insets.top}/${insets.bottom}`);
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
