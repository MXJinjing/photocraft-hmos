import { ConfigurationConstant } from '@kit.AbilityKit';

// Canonical tokens shared with the native host (`system_theme` input / init config).
export const SYSTEM_THEME_DARK: string = 'dark';
export const SYSTEM_THEME_LIGHT: string = 'light';

// Map HarmonyOS Configuration.colorMode to the host token. Unset / unknown → empty.
export function systemThemeFromColorMode(colorMode: number | undefined): string {
  if (colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK) {
    return SYSTEM_THEME_DARK;
  }
  if (colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT) {
    return SYSTEM_THEME_LIGHT;
  }
  return '';
}

export function publishSystemTheme(theme: string): void {
  AppStorage.setOrCreate('photocraftSystemTheme', theme);
}
