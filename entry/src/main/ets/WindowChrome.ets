import { window } from '@kit.ArkUI';
import { BusinessError, deviceInfo } from '@kit.BasicServicesKit';

const TITLE_HEIGHT: number = 38;

export class WindowChrome {
  private win: window.Window;
  private watching: boolean = false;
  private retries: number[] = [];
  // 矩形事件可早于窗口状态更新；先采用事件值，再延迟查询稳定后的几何信息。
  private rectChanged = (rect: window.TitleButtonRect): void => { this.publish(rect); this.refresh(); };
  private statusChanged = (): void => { this.refresh(); };

  constructor(win: window.Window) { this.win = win; }

  start(): void {
    if (deviceInfo.deviceType !== '2in1' && deviceInfo.deviceType !== 'tablet') { return; }
    try {
      // Keep the native caption buttons and border; merge content into the title bar.
      this.win.setWindowDecorHeight(TITLE_HEIGHT);
      this.win.setWindowDecorVisible(false);
      // Let the system own dragging and double-click maximize for the integrated caption area.
      this.win.setWindowTitleMoveEnabled(true);
      this.win.on('windowTitleButtonRectChange', this.rectChanged);
      this.win.on('windowStatusChange', this.statusChanged);
      this.win.on('windowSizeChange', this.statusChanged);
      this.watching = true;
      this.refresh();
    } catch (error) {
      let failure = error as BusinessError;
      console.info(`Integrated title bar unavailable: ${failure.code} ${failure.message}`);
      this.clear();
    }
  }

  refresh(): void {
    if (!this.watching) { return; }
    this.cancelRetries();
    this.readRect();
    // Window status, size and caption geometry can arrive in separate transactions.
    for (let delay of [50, 200, 600]) {
      this.retries.push(setTimeout((): void => { if (this.watching) { this.readRect(); } }, delay));
    }
  }

  private readRect(): void {
    try { this.publish(this.win.getTitleButtonRect()); }
    catch (error) { /* A transient query failure must not erase the last valid inset. */ }
  }

  private cancelRetries(): void {
    // 新一轮刷新替换旧计时器，销毁窗口时也调用，避免回调访问已失效的 Window。
    for (let timer of this.retries) { clearTimeout(timer); }
    this.retries = [];
  }

  private publish(rect: window.TitleButtonRect): void {
    let status = this.win.getWindowStatus();
    let free = status === window.WindowStatusType.FLOATING || status === window.WindowStatusType.MAXIMIZE;
    let visible = Number.isFinite(rect.width) && Number.isFinite(rect.height) && rect.width > 0 && rect.height > 0;
    // A valid caption rectangle is authoritative even when the status event is lagging.
    // Empty geometry during a floating/maximized transition retains the previous inset.
    if (!visible && free) { return; }
    let right = visible ? rect.width + 8 : 0;
    let height = visible ? TITLE_HEIGHT : 0;
    AppStorage.setOrCreate('photocraftTitleRight', right);
    AppStorage.setOrCreate('photocraftTitleHeight', height);
    console.info(`PhotoCraft title bar: status=${status}, rect=${JSON.stringify(rect)}, right=${right}, height=${height}`);
  }

  private clear(): void {
    AppStorage.setOrCreate('photocraftTitleRight', 0);
    AppStorage.setOrCreate('photocraftTitleHeight', 0);
  }

  stop(): void {
    this.cancelRetries();
    if (!this.watching) { return; }
    this.win.off('windowTitleButtonRectChange', this.rectChanged);
    this.win.off('windowStatusChange', this.statusChanged);
    this.win.off('windowSizeChange', this.statusChanged);
    this.watching = false;
    this.clear();
  }
}

export function moveWindow(win: window.Window): Promise<void> { return win.startMoving(); }

export function toggleWindowMaximize(win: window.Window): Promise<void> {
  return win.getWindowStatus() === window.WindowStatusType.FLOATING ?
    win.maximize(window.MaximizePresentation.EXIT_IMMERSIVE) : win.recover();
}
