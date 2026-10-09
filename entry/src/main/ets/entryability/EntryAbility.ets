import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { LOADING_CHROME, paintWindow } from '../SystemBars';
import { acceptOpenWant } from '../openInbox';
import { requestWindowClose, resetCloseGuard } from '../closeGuard';

export default class EntryAbility extends UIAbility {
  onCreate(want: Want, _launchParam: AbilityConstant.LaunchParam): void {
    resetCloseGuard();
    this.noteWant(want);
    console.info('PhotoCraft ability created');
  }

  onNewWant(want: Want): void {
    this.noteWant(want);
  }

  private noteWant(want: Want): void {
    AppStorage.setOrCreate('photocraftDev', want.parameters?.['photocraft.dev'] === true);
    AppStorage.setOrCreate('photocraftGpu', want.parameters?.['photocraft.gpu'] === true);
    acceptOpenWant(want);
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    windowStage.on('windowStageClose', () => requestWindowClose());
    AppStorage.setOrCreate('photocraftChrome', LOADING_CHROME);
    windowStage.loadContent('pages/Index', (error: BusinessError) => {
      if (error.code) {
        console.error(`PhotoCraft page failed: ${error.code} ${error.message}`);
        return;
      }
      // The page does not draw into the status-bar and navigation-indicator avoid areas. Colour
      // those from the window, then let the page replace this with the theme chrome.
      try {
        paintWindow(windowStage.getMainWindowSync(), LOADING_CHROME);
      } catch (err) {
        let failure = err as BusinessError;
        console.error(`System bars failed: ${failure.code} ${failure.message}`);
      }
    });
  }
}
