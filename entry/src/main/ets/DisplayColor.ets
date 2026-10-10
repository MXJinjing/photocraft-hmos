import { common } from '@kit.AbilityKit';
import { display, window } from '@kit.ArkUI';
import { colorSpaceManager } from '@kit.ArkGraphics2D';

// This is the application output space, not a measured panel ICC profile.
// HarmonyOS performs the final mapping from the tagged surface to the panel.
export async function configureDisplayColor(context: common.UIAbilityContext): Promise<boolean> {
  try {
    const screen = display.getDefaultDisplaySync();
    const spaces = screen.colorSpaces ?? [];
    const win = await window.getLastWindow(context);
    const wide = await win.isWindowSupportWideGamut();
    // Some systems leave the display list empty. The window's capability query
    // still permits negotiation, followed by both window and NativeWindow readback.
    const p3 = wide && (spaces.length === 0 ||
      spaces.includes(colorSpaceManager.ColorSpace.DISPLAY_P3) ||
      spaces.includes(colorSpaceManager.ColorSpace.DCI_P3));
    await win.setWindowColorSpace(p3 ? window.ColorSpace.WIDE_GAMUT : window.ColorSpace.DEFAULT);
    const enabled = p3 && win.getWindowColorSpace() === window.ColorSpace.WIDE_GAMUT;
    console.info(`PhotoCraft display ${screen.name}: spaces=${JSON.stringify(spaces)}, wide=${wide}, output=${enabled ? 'Display P3' : 'sRGB'}`);
    return enabled;
  } catch (error) {
    console.warn(`PhotoCraft wide gamut unavailable, requesting sRGB: ${error}`);
    try {
      const win = await window.getLastWindow(context);
      await win.setWindowColorSpace(window.ColorSpace.DEFAULT);
    } catch (fallbackError) {
      console.warn(`PhotoCraft window gamut reset failed: ${fallbackError}`);
    }
    return false;
  }
}
