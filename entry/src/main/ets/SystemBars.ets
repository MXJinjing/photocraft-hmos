import { common } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';

// Loading page and start window. PhotoCraft replaces this with the theme chrome once it is running.
export const LOADING_CHROME: string = '#FF262626';

// `#RRGGBB` or `#AARRGGBB` from the page, or nothing if it is not a colour.
export function normalizeChrome(color: string): string {
  if (color.length === 0 || color.charAt(0) !== '#') {
    return '';
  }
  let hex: string = color.substring(1);
  if (hex.length === 3) {
    hex = hex.charAt(0) + hex.charAt(0) + hex.charAt(1) + hex.charAt(1) + hex.charAt(2) + hex.charAt(2);
  }
  if (hex.length !== 6 && hex.length !== 8) {
    return '';
  }
  for (let i = 0; i < hex.length; i++) {
    let c: string = hex.charAt(i);
    let digit: boolean = c >= '0' && c <= '9';
    let lower: boolean = c >= 'a' && c <= 'f';
    let upper: boolean = c >= 'A' && c <= 'F';
    if (!digit && !lower && !upper) {
      return '';
    }
  }
  if (hex.length === 6) {
    return '#FF' + hex.toUpperCase();
  }
  return '#' + hex.toUpperCase();
}

// White status-bar icons on a dark chrome, black icons on a light one.
function lightIcons(argb: string): boolean {
  let r: number = parseInt(argb.substring(3, 5), 16);
  let g: number = parseInt(argb.substring(5, 7), 16);
  let b: number = parseInt(argb.substring(7, 9), 16);
  let y: number = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return y < 160;
}

export function paintWindow(win: window.Window, color: string): void {
  let argb: string = normalizeChrome(color);
  if (argb.length === 0) {
    return;
  }
  let content: string = lightIcons(argb) ? '#FFFFFFFF' : '#FF1A1A1A';
  let light: boolean = content === '#FFFFFFFF';
  try {
    win.setWindowBackgroundColor(argb);
  } catch (error) {
    let failure = error as BusinessError;
    console.error(`Window background failed: ${failure.code} ${failure.message}`);
  }
  let props: window.SystemBarProperties = {
    statusBarColor: argb,
    navigationBarColor: argb,
    statusBarContentColor: content,
    navigationBarContentColor: content,
    isStatusBarLightIcon: light,
    isNavigationBarLightIcon: light
  };
  win.setWindowSystemBarProperties(props).catch((error: BusinessError) => {
    console.error(`System bar properties failed: ${error.code} ${error.message}`);
  });
}

export function applySystemBars(ctx: common.UIAbilityContext, color: string): void {
  let argb: string = normalizeChrome(color);
  if (argb.length === 0) {
    return;
  }
  window.getLastWindow(ctx).then((win: window.Window) => {
    paintWindow(win, argb);
  }).catch((error: BusinessError) => {
    console.error(`System bars failed: ${error.code} ${error.message}`);
  });
}
