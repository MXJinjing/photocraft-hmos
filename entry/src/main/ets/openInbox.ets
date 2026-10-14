import { Want } from '@kit.AbilityKit';
import { fileIo, fileUri } from '@kit.CoreFileKit';
import { BusinessError } from '@kit.BasicServicesKit';

const STREAM_KEY: string = 'ability.params.stream';

export class OpenedFile {
  name: string = '';
  uri: string = '';
  file: fileIo.File;

  constructor(name: string, file: fileIo.File, uri: string = '') {
    this.name = name;
    this.uri = uri;
    this.file = file;
  }
}

// 冷启动时保留已授权的文件句柄，页面就绪后交给原生层读取并关闭原句柄。
// 不在 ArkTS 主线程读取或缓存整个大文件。
let queued: Array<OpenedFile> = [];
let listener: (() => void) | null = null;

export function setOpenListener(next: (() => void) | null): void {
  listener = next;
}

export function takeOpens(): Array<OpenedFile> {
  let files = queued;
  queued = [];
  return files;
}

export function acceptOpenWant(want: Want): void {
  let uris = fileUris(want);
  for (let uri of uris) {
    enqueueUri(uri);
  }
}

function fileUris(want: Want): Array<string> {
  let uris: Array<string> = [];
  if (want.uri !== undefined && want.uri.length > 0) {
    uris.push(want.uri);
  }
  let params = want.parameters;
  if (params === undefined) {
    return uris;
  }
  let stream = params[STREAM_KEY];
  if (!(stream instanceof Array)) {
    return uris;
  }
  let list = stream as Array<string>;
  for (let uri of list) {
    if (uri.length > 0 && uris.indexOf(uri) < 0) {
      uris.push(uri);
    }
  }
  return uris;
}

function enqueueUri(uri: string): void {
  let file: fileIo.File | null = null;
  try {
    file = fileIo.openSync(uri, fileIo.OpenMode.READ_ONLY);
    queued.push(new OpenedFile(displayName(uri), file, uri));
    file = null; // Ownership moves to the inbox consumer.
    if (listener !== null) {
      listener();
    }
  } catch (error) {
    let failure = error as BusinessError;
    console.error(`Open failed ${uri}: ${failure.code} ${failure.message}`);
  } finally {
    if (file !== null) {
      try {
        fileIo.closeSync(file);
      } catch (error) {
        let failure = error as BusinessError;
        console.error(`Open close failed: ${failure.code} ${failure.message}`);
      }
    }
  }
}

function displayName(uri: string): string {
  try {
    let name = new fileUri.FileUri(uri).name;
    if (name.length > 0) {
      return name;
    }
  } catch (error) {
    let failure = error as BusinessError;
    console.error(`Open name failed: ${failure.code} ${failure.message}`);
  }
  let cut = uri.lastIndexOf('/');
  let name = cut >= 0 ? uri.substring(cut + 1) : uri;
  let query = name.indexOf('?');
  if (query >= 0) {
    name = name.substring(0, query);
  }
  if (name.length === 0) {
    return 'opened';
  }
  try {
    return decodeURIComponent(name);
  } catch (error) {
    return name;
  }
}
