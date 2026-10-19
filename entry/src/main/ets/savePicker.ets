import { picker } from '@kit.CoreFileKit';
import { saveFormatChoices } from './saveFormats';

export function savePickerOptions(suggested: string, directoryUri: string): picker.DocumentSaveOptions {
  let name = suggested.substring(suggested.lastIndexOf('/') + 1).replace(/[\\:*?"<>|\x00-\x1F\x7F]/g, '_');
  let options = new picker.DocumentSaveOptions();
  options.newFileNames = [name];
  options.fileSuffixChoices = saveFormatChoices(name);
  // DEFAULT displays the full system picker. DOWNLOAD would skip the UI and ignore this URI.
  // Empty when DOWNLOAD authorization failed at startup: the picker's own default then.
  if (directoryUri.length > 0) { options.defaultFilePathUri = directoryUri; }
  return options;
}

/**
 * Maps a durable save key (app download path, persisted file:// URI, or legacy
 * save-default/save-as token) to the Picker-granted URI used for CoreFileKit I/O.
 */
export class SaveTargets {
  private targets: Map<string, string> = new Map();
  /** Ordinary Save retains the grant for repeat Ctrl+S; Save As is one-shot. */
  private retain: Map<string, boolean> = new Map();

  /**
   * @param durable When set (persisted URI or owned filesystem path), Recent / Save use that
   *   key instead of a session-only save-default/ / save-as/ token.
   */
  select(id: number, name: string, uri: string, ordinary: boolean, durable?: string): string {
    let target = (durable !== undefined && durable.length > 0)
      ? durable
      : ((ordinary ? 'save-default/' : 'save-as/') + id.toString() + '/' + name);
    this.targets.set(target, uri);
    this.retain.set(target, ordinary);
    return target;
  }

  take(target: string): string | undefined {
    let uri = this.targets.get(target);
    if (uri !== undefined && this.retain.get(target) !== true) {
      this.targets.delete(target);
      this.retain.delete(target);
    }
    return uri;
  }

  clear(): void {
    this.targets.clear();
    this.retain.clear();
  }
}
