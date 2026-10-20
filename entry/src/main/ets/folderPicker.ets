import { common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';

function safeName(name: string): string {
  let base = name.substring(name.lastIndexOf('/') + 1).replace(/[\\:*?"<>|\x00-\x1F\x7F]/g, '_').trim();
  return base.length === 0 || base === '.' || base === '..' ? 'file' : base;
}

/** Ensures `documents/<sub>` exists and returns that path. Empty `sub` → documents root. */
export function ensureDocumentSubdir(documents: string, sub: string): string {
  if (documents.length === 0) {
    throw new Error('This device has no folder picker and no document folder');
  }
  if (sub.length === 0) {
    return documents;
  }
  let dir = documents + '/' + sub.replace(/^\/+/, '');
  if (!fileIo.accessSync(dir)) {
    fileIo.mkdirSync(dir, true);
  }
  return dir;
}

/**
 * Tries the system folder picker (DocumentSelectMode.FOLDER). Returns the path, '' if the user
 * cancelled, or null when the device cannot show folder selection.
 */
async function trySystemFolder(context: common.UIAbilityContext): Promise<string | null> {
  // Prefer the FolderSelection syscap when advertised (2in1/PC).
  let advertised = canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection');
  try {
    let options = new picker.DocumentSelectOptions();
    options.selectMode = picker.DocumentSelectMode.FOLDER;
    options.maxSelectNumber = 1;
    let uris = await new picker.DocumentViewPicker(context).select(options);
    if (uris.length === 0) {
      return '';
    }
    let path = new fileUri.FileUri(uris[0]).path;
    if (path.length === 0) {
      throw new Error('The chosen folder has no path');
    }
    return path;
  } catch (error) {
    // Tablets (and any device without FolderSelection) reject FOLDER mode.
    console.info(`PhotoCraft folder select unavailable (advertised=${advertised}): ${(error as Error).message}`);
    return null;
  }
}

/**
 * Answers a native "folder" request with a path the Rust engine can use, or '' when cancelled.
 * - 2in1/PC (FolderSelection): system file manager in FOLDER mode.
 * - Tablet / no FolderSelection, input: multi-file pick; files are copied into filesDir/picked/<time>
 *   (there is no system directory picker — never silently open a single-file picker).
 * - Tablet / no FolderSelection, output: the app DOWNLOAD document folder (`~/Downloads/<bundle>`),
 *   optionally a named subfolder (`processed`, `exports`, …) from `preferredSubdir`.
 */
export async function pickFolder(
  context: common.UIAbilityContext,
  output: boolean,
  documents: string,
  preferredSubdir: string = ''
): Promise<string> {
  let system = await trySystemFolder(context);
  if (system !== null) {
    return system;
  }
  if (output) {
    // Explicit degradation: no folder UI — use the DOWNLOAD package directory (or subfolder).
    return ensureDocumentSubdir(documents, preferredSubdir);
  }
  // Input without a folder picker: multi-select files and stage them as a sandbox folder.
  let options = new picker.DocumentSelectOptions();
  options.maxSelectNumber = 500;
  let uris = await new picker.DocumentViewPicker(context).select(options);
  if (uris.length === 0) {
    return '';
  }
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
    } finally {
      await fileIo.close(file);
    }
  }
  return dir;
}

// Startup must not fail when DOWNLOAD authorization does: fall back to the sandbox.
export function sandboxDocumentDirectory(context: common.UIAbilityContext): string {
  let path = context.filesDir + '/Documents';
  if (!fileIo.accessSync(path)) {
    fileIo.mkdirSync(path, true);
  }
  return path;
}

/** Creates the standard output subfolders under the DOWNLOAD document directory. */
export function ensureOutputSubdirs(documents: string): void {
  if (documents.length === 0) {
    return;
  }
  const subs: string[] = ['processed', 'batch', 'corrected', 'exports', 'droplet-output', 'package'];
  for (let sub of subs) {
    ensureDocumentSubdir(documents, sub);
  }
}
