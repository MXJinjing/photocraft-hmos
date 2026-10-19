import { common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';

function safeName(name: string): string {
  let base = name.substring(name.lastIndexOf('/') + 1).replace(/[\\:*?"<>|\x00-\x1F\x7F]/g, '_').trim();
  return base.length === 0 || base === '.' || base === '..' ? 'file' : base;
}

/**
 * Answers a native "folder" request with a path the Rust engine can use, or '' when cancelled.
 * - 2in1/PC: the system file manager in FOLDER mode; the granted folder's path.
 * - Tablet, input: the file manager (multi-select); the files are copied into a new folder in
 *   the sandbox (filesDir/picked/<time>) and that folder is the answer.
 * - Tablet, output: no folder can be granted, so the app's system-visible document folder.
 */
export async function pickFolder(context: common.UIAbilityContext, output: boolean, documents: string): Promise<string> {
  // System folder selection exists only on 2in1/PC (FolderSelection syscap).
  if (canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection')) {
    let options = new picker.DocumentSelectOptions();
    options.selectMode = picker.DocumentSelectMode.FOLDER;
    options.maxSelectNumber = 1;
    let uris = await new picker.DocumentViewPicker(context).select(options);
    if (uris.length === 0) { return ''; }
    let path = new fileUri.FileUri(uris[0]).path;
    if (path.length === 0) { throw new Error('The chosen folder has no path'); }
    return path;
  }
  if (output) {
    if (documents.length === 0) { throw new Error('This device has no folder picker and no document folder'); }
    return documents;
  }
  let options = new picker.DocumentSelectOptions();
  options.maxSelectNumber = 500;
  let uris = await new picker.DocumentViewPicker(context).select(options);
  if (uris.length === 0) { return ''; }
  let dir = context.filesDir + '/picked/' + Date.now().toString();
  await fileIo.mkdir(dir, true);
  let used: Set<string> = new Set<string>();
  for (let uri of uris) {
    let name = safeName(new fileUri.FileUri(uri).name);
    let unique = name;
    for (let n = 1; used.has(unique); n++) {
      let dot = name.lastIndexOf('.');
      unique = dot > 0 ? name.substring(0, dot) + ' ' + n.toString() + name.substring(dot) : name + ' ' + n.toString();
    }
    used.add(unique);
    let file = await fileIo.open(uri, fileIo.OpenMode.READ_ONLY);
    try {
      await fileIo.copyFile(file.fd, dir + '/' + unique);
    } finally { await fileIo.close(file); }
  }
  return dir;
}

// Startup must not fail when DOWNLOAD authorization does: fall back to the sandbox.
export function sandboxDocumentDirectory(context: common.UIAbilityContext): string {
  let path = context.filesDir + '/Documents';
  if (!fileIo.accessSync(path)) { fileIo.mkdirSync(path, true); }
  return path;
}
