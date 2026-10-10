import native from 'libphotocraft.so';
import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { applySystemBars, LOADING_CHROME, paintWindow } from '../SystemBars';
import { restorePointerVisibility } from '../input/NativeCursorAdapter';
import { acceptOpenWant } from '../openInbox';
import { requestWindowClose, resetCloseGuard } from '../closeGuard';
import { WindowChrome } from '../WindowChrome';

export default class EntryAbility extends UIAbility {
  private windowChrome: WindowChrome | undefined = undefined;
  onCreate(want: Want, _launchParam: AbilityConstant.LaunchParam): void {
    resetCloseGuard();
    this.noteWant(want);
    console.info('PhotoCraft native ability created');
  }

  onNewWant(want: Want): void {
    this.noteWant(want);
  }

  private noteWant(want: Want): void {
    acceptOpenWant(want);
  }

  onForeground(): void {
    // 系统窗口在后台可能改变装饰和颜色，恢复前台时重新查询并唤醒渲染 worker。
    this.windowChrome?.refresh();
    applySystemBars(this.context, AppStorage.get<string>('photocraftChrome') ?? LOADING_CHROME);
    native.active(true);
  }

  onBackground(): void { restorePointerVisibility(); native.active(false); }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    windowStage.on('windowStageClose', () => requestWindowClose());
    if (AppStorage.get<string>('photocraftChrome') === undefined) {
      AppStorage.setOrCreate('photocraftChrome', LOADING_CHROME);
    }
    windowStage.loadContent('pages/Index', (error: BusinessError) => {
      if (error.code) {
        console.error(`PhotoCraft page failed: ${error.code} ${error.message}`);
        return;
      }
      // The page does not draw into the status-bar and navigation-indicator avoid areas. Colour
      // those from the window, then let the page replace this with the theme chrome.
      try {
        let win = windowStage.getMainWindowSync();
        paintWindow(win, AppStorage.get<string>('photocraftChrome') ?? LOADING_CHROME);
        this.windowChrome?.stop();
        this.windowChrome = new WindowChrome(win);
        this.windowChrome.start();
      } catch (err) {
        let failure = err as BusinessError;
        console.error(`System bars failed: ${failure.code} ${failure.message}`);
      }
    });
  }

  onWindowStageDestroy(): void { this.windowChrome?.stop(); this.windowChrome = undefined; }
}
