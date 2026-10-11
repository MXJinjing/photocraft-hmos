import native from 'libphotocraft.so';
import { AbilityConstant, Configuration, UIAbility, Want } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { applySystemBars, LOADING_CHROME, paintWindow } from '../SystemBars';
import { publishSystemTheme, systemThemeFromColorMode } from '../SystemTheme';
import { restorePointerVisibility } from '../input/NativeCursorAdapter';
import { acceptOpenWant } from '../openInbox';
import { requestWindowClose, resetCloseGuard } from '../closeGuard';
import { WindowChrome } from '../WindowChrome';

class SystemThemeInput {
  kind: string = 'system_theme';
  text: string = '';
  constructor(text: string) { this.text = text; }
}

export default class EntryAbility extends UIAbility {
  private windowChrome: WindowChrome | undefined = undefined;

  private syncSystemTheme(colorMode: number | undefined): void {
    let theme = systemThemeFromColorMode(colorMode);
    publishSystemTheme(theme);
    // Push live changes into the native worker; init also carries the first value.
    try {
      native.input(JSON.stringify(new SystemThemeInput(theme)));
    } catch (error) {
      console.info('System theme input unavailable before native init');
    }
  }

  onCreate(want: Want, _launchParam: AbilityConstant.LaunchParam): void {
    resetCloseGuard();
    this.noteWant(want);
    this.syncSystemTheme(this.context.config?.colorMode);
    console.info('PhotoCraft native ability created');
  }

  onNewWant(want: Want): void {
    this.noteWant(want);
  }

  private noteWant(want: Want): void {
    acceptOpenWant(want);
  }

  onConfigurationUpdate(newConfig: Configuration): void {
    this.syncSystemTheme(newConfig.colorMode);
  }

  onForeground(): void {
    // 系统窗口在后台可能改变装饰和颜色，恢复前台时重新查询并唤醒渲染 worker。
    this.windowChrome?.refresh();
    applySystemBars(this.context, AppStorage.get<string>('photocraftChrome') ?? LOADING_CHROME);
    // Re-read in case the colour scheme changed while we were backgrounded without a config event.
    this.syncSystemTheme(this.context.config?.colorMode);
    native.active(true);
  }

  onBackground(): void { restorePointerVisibility(); native.active(false); }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    windowStage.on('windowStageClose', () => requestWindowClose());
    if (AppStorage.get<string>('photocraftChrome') === undefined) {
      AppStorage.setOrCreate('photocraftChrome', LOADING_CHROME);
    }
    if (AppStorage.get<string>('photocraftSystemTheme') === undefined) {
      this.syncSystemTheme(this.context.config?.colorMode);
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
