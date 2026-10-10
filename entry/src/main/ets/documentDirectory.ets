import { common } from '@kit.AbilityKit';
import { fileUri, picker } from '@kit.CoreFileKit';

// API 12 DOWNLOAD mode returns a persistent, system-visible application directory
// without displaying a location picker. Do not construct or hard-code a public path.
export async function documentDirectory(context: common.UIAbilityContext): Promise<string> {
  // 下载目录由系统分配给当前包；它与 filesDir 沙箱及脚本工作目录相互独立。
  let options = new picker.DocumentSaveOptions();
  options.pickerMode = picker.DocumentPickerMode.DOWNLOAD;
  let uris = await new picker.DocumentViewPicker(context).save(options);
  if (uris.length === 0) { throw new Error('Unable to obtain the application document directory'); }
  let path = new fileUri.FileUri(uris[0]).path;
  if (path.length === 0) { throw new Error('Application document directory has no path'); }
  return path;
}
