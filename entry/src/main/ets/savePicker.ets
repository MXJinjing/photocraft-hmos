import { picker } from '@kit.CoreFileKit';
import { saveFormatChoices } from './saveFormats';

export function savePickerOptions(suggested: string, directoryUri: string): picker.DocumentSaveOptions {
  let name = suggested.substring(suggested.lastIndexOf('/') + 1).replace(/[\\:*?"<>|\x00-\x1F\x7F]/g, '_');
  let options = new picker.DocumentSaveOptions();
  options.newFileNames = [name];
  options.fileSuffixChoices = saveFormatChoices(name);
  // DEFAULT displays the full system picker. DOWNLOAD would skip the UI and ignore this URI.
  options.defaultFilePathUri = directoryUri;
  return options;
}

export class SaveTargets {
  private targets: Map<string, string> = new Map();
  select(id: number, name: string, uri: string, ordinary: boolean): string {
    let target = (ordinary ? 'save-default/' : 'save-as/') + id.toString() + '/' + name;
    this.targets.set(target, uri);
    return target;
  }
  take(target: string): string | undefined {
    let uri = this.targets.get(target);
    if (!target.startsWith('save-default/')) this.targets.delete(target);
    return uri;
  }
  clear(): void { this.targets.clear(); }
}
