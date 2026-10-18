import { common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';
import { util } from '@kit.ArkTS';

const CACHE_NAME: string = 'photocraft-document-uri.txt';
// Emulator/2in1 has shown downloadAuth hanging on DOWNLOAD mode and tripping
// APP_INPUT_BLOCK (~8s). Bound the wait so native.init can proceed.
const DOWNLOAD_TIMEOUT_MS: number = 3000;

// API 12 DOWNLOAD mode returns a persistent, system-visible application directory
// without displaying a location picker. Do not construct or hard-code a public path.
export async function documentDirectory(context: common.UIAbilityContext): Promise<string> {
  let uri = await applicationDocumentUri(context);
  let path = new fileUri.FileUri(uri).path;
  if (path.length === 0) { throw new Error('Application document directory has no path'); }
  return path;
}

export async function applicationDocumentUri(context: common.UIAbilityContext): Promise<string> {
  const cached = readCachedUri(context);
  if (cached !== undefined) {
    console.info('Document directory: using cached URI');
    return cached;
  }
  try {
    const uri = await withTimeout(discoverDownloadUri(context), DOWNLOAD_TIMEOUT_MS);
    writeCachedUri(context, uri);
    console.info('Document directory: DOWNLOAD mode OK');
    return uri;
  } catch (error) {
    console.error(`Document directory DOWNLOAD failed/timeout: ${(error as Error).message}`);
    // Do not cache the sandbox fallback: a later launch may get a real DOWNLOAD URI.
    console.info('Document directory: falling back to sandbox Documents');
    return sandboxDocumentsUri(context);
  }
}

async function discoverDownloadUri(context: common.UIAbilityContext): Promise<string> {
  // 下载目录由系统分配给当前包；它与 filesDir 沙箱及脚本工作目录相互独立。
  let options = new picker.DocumentSaveOptions();
  options.pickerMode = picker.DocumentPickerMode.DOWNLOAD;
  let uris = await new picker.DocumentViewPicker(context).save(options);
  if (uris.length === 0) { throw new Error('Unable to obtain the application document directory'); }
  return uris[0];
}

function sandboxDocumentsUri(context: common.UIAbilityContext): string {
  const path = `${context.filesDir}/Documents`;
  try {
    fileIo.mkdirSync(path);
  } catch (_error) {
    // Already exists, or will fail on first write — surface that later.
  }
  return fileUri.getUriFromPath(path);
}

function cachePath(context: common.UIAbilityContext): string {
  return `${context.filesDir}/${CACHE_NAME}`;
}

function readCachedUri(context: common.UIAbilityContext): string | undefined {
  try {
    const uri = fileIo.readTextSync(cachePath(context)).trim();
    return uri.length > 0 ? uri : undefined;
  } catch (_error) {
    return undefined;
  }
}

function writeCachedUri(context: common.UIAbilityContext, uri: string): void {
  try {
    const file = fileIo.openSync(
      cachePath(context),
      fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE | fileIo.OpenMode.TRUNC
    );
    try {
      const encoder = new util.TextEncoder();
      const bytes = encoder.encodeInto(uri);
      fileIo.writeSync(file.fd, bytes.buffer);
    } finally {
      fileIo.closeSync(file);
    }
  } catch (error) {
    console.info(`Document URI cache write skipped: ${(error as Error).message}`);
  }
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout((): void => {
      reject(new Error(`DocumentViewPicker DOWNLOAD timed out after ${ms}ms`));
    }, ms);
    promise.then((value: T): void => {
      clearTimeout(timer);
      resolve(value);
    }).catch((error: Error): void => {
      clearTimeout(timer);
      reject(error);
    });
  });
}
