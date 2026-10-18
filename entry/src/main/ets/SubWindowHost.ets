import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { toggleWindowMaximize } from './WindowChrome';

// Floating document windows (upstream 0.9.0 `show_viewport_immediate`) are native HarmonyOS
// sub-windows: each has system decoration and its own XComponent surface. Rust emits
// `window` notifications ({op, win, …}); this host creates/updates/destroys the windows.
export const SUB_WINDOW_PAGE: string = 'pages/SubWindow';
export const SUB_WINDOW_KEY: string = 'winKey';

// Set immediately before loadContent so SubWindow.aboutToAppear can read a reliable key
// even when @LocalStorageProp is still 0 on the first frame (that used to yield pc-win-0
// and a permanently black XComponent).
let pendingSubWindowKey: number = 0;

export function setPendingSubWindowKey(key: number): void {
  pendingSubWindowKey = key > 0 ? key : 0;
}

export function takePendingSubWindowKey(): number {
  const key = pendingSubWindowKey;
  pendingSubWindowKey = 0;
  return key;
}

export class WindowRequest {
  op: string = '';
  win: number = 0;
  title: string = '';
  width: number = 0;
  height: number = 0;
  x: number | null = null;
  y: number | null = null;
}

class WindowClose {
  kind: string = 'win_close';
  win: number = 0;
  constructor(win: number) { this.win = win; }
}

export function subWindowName(key: number): string {
  return `pc-doc-${key}`;
}

export function subWindowOptions(title: string): window.SubWindowOptions {
  // decorEnabled: system title bar with minimize / maximize / close in free-window state.
  // maximizeSupported (API 19+): show the maximize button; still needs floating support.
  const options: window.SubWindowOptions = { title: title, decorEnabled: true, maximizeSupported: true };
  return options;
}

export function closeMessage(key: number): string {
  return JSON.stringify(new WindowClose(key));
}

// Tag an input message (already JSON) with the sub-window it belongs to.
export function taggedForWindow(message: string, key: number): string {
  if (key <= 0 || message.length < 2 || message.charAt(0) !== '{') { return message; }
  return `{"win":${key},${message.substring(1)}`;
}

export class SubWindowHost {
  private stage: window.WindowStage | undefined = undefined;
  private send: (message: string) => void = (_message: string): void => {};
  private windows: Map<number, window.Window> = new Map<number, window.Window>();
  // Close arrived while the window was still being created.
  private closed: Set<number> = new Set<number>();
  // Keep listeners so close() can off() them.
  private statusHandlers: Map<number, (status: window.WindowStatusType) => void> =
    new Map<number, (status: window.WindowStatusType) => void>();

  attach(stage: window.WindowStage, send: (message: string) => void): void {
    this.stage = stage;
    this.send = send;
  }

  handle(request: WindowRequest): void {
    if (request.op === 'open') {
      this.open(request).catch((error: BusinessError): void => this.openFailed(request.win, error));
    } else if (request.op === 'close') {
      this.close(request.win);
    } else if (request.op === 'title') {
      this.windows.get(request.win)?.setWindowTitle(request.title).catch((error: BusinessError): void => {
        console.info(`Sub-window title: ${error.code} ${error.message}`);
      });
    } else if (request.op === 'resize') {
      this.windows.get(request.win)?.resize(request.width, request.height).catch((error: BusinessError): void => {
        console.info(`Sub-window resize: ${error.code} ${error.message}`);
      });
    } else if (request.op === 'maximize') {
      this.toggleMaximize(request.win);
    }
  }

  count(): number {
    return this.windows.size;
  }

  closeAll(): void {
    const keys: number[] = Array.from(this.windows.keys());
    for (const key of keys) {
      this.unwatch(key);
    }
    const all: window.Window[] = Array.from(this.windows.values());
    this.windows.clear();
    this.closed.clear();
    for (const win of all) {
      win.destroyWindow().catch((error: BusinessError): void => {
        console.info(`Sub-window destroy: ${error.code} ${error.message}`);
      });
    }
  }

  private async open(request: WindowRequest): Promise<void> {
    const key = request.win;
    const stage = this.stage;
    if (stage === undefined) { throw new Error('window stage unavailable'); }
    this.closed.delete(key);
    const win: window.Window = await stage.createSubWindowWithOptions(subWindowName(key), subWindowOptions(request.title));
    if (this.closed.delete(key)) {
      await win.destroyWindow();
      return;
    }
    this.windows.set(key, win);
    // The system close button only asks: Rust closes the viewport and then tells us to destroy.
    win.on('windowWillClose', (): Promise<boolean> => {
      this.send(closeMessage(key));
      return Promise.resolve(true);
    });
    win.on('windowEvent', (event: window.WindowEventType): void => {
      if (event === window.WindowEventType.WINDOW_DESTROYED && this.windows.has(key)) {
        this.unwatch(key);
        this.windows.delete(key);
        this.send(closeMessage(key));
      }
    });
    // When the system maximize/recover actually changes status, refresh size so the
    // XComponent surface tracks the chrome (SurfaceChanged alone is not always enough).
    const onStatus = (status: window.WindowStatusType): void => {
      this.onStatusChange(key, status);
    };
    this.statusHandlers.set(key, onStatus);
    win.on('windowStatusChange', onStatus);
    await win.resize(request.width, request.height);
    if (request.x !== null && request.y !== null) {
      await win.moveWindowTo(request.x, request.y);
    }
    try {
      win.setWindowDecorVisible(true);
      win.setWindowTitleButtonVisible(true, true, true);
    } catch (error) {
      console.info(`Sub-window decor buttons: ${(error as BusinessError).code} ${(error as BusinessError).message}`);
    }
    const storage = new LocalStorage();
    storage.setOrCreate(SUB_WINDOW_KEY, key);
    // Must be set before loadContent so aboutToAppear sees a non-zero key.
    setPendingSubWindowKey(key);
    await win.loadContent(SUB_WINDOW_PAGE, storage);
    await win.showWindow();
    // A close that raced with showWindow.
    if (this.closed.delete(key)) {
      this.close(key);
    }
  }

  private onStatusChange(key: number, status: window.WindowStatusType): void {
    const win = this.windows.get(key);
    const stage = this.stage;
    if (win === undefined || stage === undefined) { return; }
    console.info(`Sub-window ${key} status=${status}`);
    if (status === window.WindowStatusType.MAXIMIZE || status === window.WindowStatusType.FULL_SCREEN) {
      stage.getMainWindow().then((main: window.Window): void => {
        const rect = main.getWindowProperties().windowRect;
        win.resize(Math.max(1, rect.width), Math.max(1, rect.height)).catch((error: BusinessError): void => {
          console.info(`Sub-window maximize resize: ${error.code} ${error.message}`);
        });
      }).catch((error: BusinessError): void => {
        console.info(`Sub-window main window: ${error.code} ${error.message}`);
      });
    }
  }

  // Explicit maximize/recover (system title-bar maximize is a no-op on some 2in1 builds
  // when the parent is not free-floating). Rust can also emit op:"maximize".
  private toggleMaximize(key: number): void {
    const win = this.windows.get(key);
    if (win === undefined) { return; }
    toggleWindowMaximize(win).catch((error: BusinessError): void => {
      console.info(`Sub-window maximize: ${error.code} ${error.message}`);
    });
  }

  private unwatch(key: number): void {
    const win = this.windows.get(key);
    const handler = this.statusHandlers.get(key);
    if (win !== undefined && handler !== undefined) {
      try { win.off('windowStatusChange', handler); } catch (_error) { /* already gone */ }
    }
    this.statusHandlers.delete(key);
  }

  private openFailed(key: number, error: BusinessError): void {
    console.error(`Sub-window ${key} failed: ${error.code} ${error.message}`);
    this.unwatch(key);
    const win = this.windows.get(key);
    this.windows.delete(key);
    win?.destroyWindow().catch((_error: BusinessError): void => {});
    setPendingSubWindowKey(0);
    // Let the app drop the viewport instead of waiting for a surface that will not come.
    this.send(closeMessage(key));
  }

  private close(key: number): void {
    const win = this.windows.get(key);
    if (win === undefined) {
      this.closed.add(key);
      return;
    }
    this.unwatch(key);
    this.windows.delete(key);
    win.destroyWindow().catch((error: BusinessError): void => {
      console.info(`Sub-window destroy: ${error.code} ${error.message}`);
    });
  }
}

export const subWindowHost: SubWindowHost = new SubWindowHost();
