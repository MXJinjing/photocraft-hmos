import { Want } from '@kit.AbilityKit';
import { fileIo, fileUri } from '@kit.CoreFileKit';
import { BusinessError } from '@kit.BasicServicesKit';

const MAX_OPEN_BYTES: number = 256 * 1024 * 1024;
const STREAM_KEY: string = 'ability.params.stream';

export class OpenedFile {
  name: string = '';
  bytes: ArrayBuffer = new ArrayBuffer(0);

  constructor(name: string, bytes: ArrayBuffer) {
    this.name = name;
    this.bytes = bytes;
  }
}

// 冷启动的打开请求可能早于编辑器初始化，先保留文件内容，待页面就绪后统一消费。
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
    let size = fileIo.statSync(file.fd).size;
    if (size <= 0 || size > MAX_OPEN_BYTES) {
      console.error(`Open skipped ${uri}: size ${size}`);
      return;
    }
    let bytes = readAll(file.fd, size);
    if (bytes === null) {
      console.error(`Open read incomplete: ${uri}`);
      return;
    }
    queued.push(new OpenedFile(displayName(uri), bytes));
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

function readAll(fd: number, size: number): ArrayBuffer | null {
  let bytes = new ArrayBuffer(size);
  let view = new Uint8Array(bytes);
  let offset = 0;
  while (offset < size) {
    let chunk = new ArrayBuffer(Math.min(1024 * 1024, size - offset));
    let read = fileIo.readSync(fd, chunk);
    if (read <= 0) {
      return null;
    }
    view.set(new Uint8Array(chunk, 0, read), offset);
    offset += read;
  }
  return bytes;
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
