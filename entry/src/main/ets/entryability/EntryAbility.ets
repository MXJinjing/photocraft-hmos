import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';

export default class EntryAbility extends UIAbility {
  onCreate(want: Want, _launchParam: AbilityConstant.LaunchParam): void {
    AppStorage.setOrCreate('photocraftDev', want.parameters?.['photocraft.dev'] === true);
    console.info('PhotoCraft Pad ability created');
  }

  onNewWant(want: Want): void {
    AppStorage.setOrCreate('photocraftDev', want.parameters?.['photocraft.dev'] === true);
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    windowStage.loadContent('pages/Index', (error: BusinessError) => {
      if (error.code) {
        console.error(`PhotoCraft Pad page failed: ${error.code} ${error.message}`);
      }
    });
  }
}
