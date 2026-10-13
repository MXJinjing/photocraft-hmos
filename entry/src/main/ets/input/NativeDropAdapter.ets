import { unifiedDataChannel, uniformDataStruct, uniformTypeDescriptor } from '@kit.ArkData';
import { fileIo, fileUri } from '@kit.CoreFileKit';
import { image } from '@kit.ImageKit';

const UTD = uniformTypeDescriptor.UniformDataType;
export const IMAGE_DROP_TYPES: Array<string> = [UTD.IMAGE, UTD.FILE_URI, UTD.FILE, UTD.OPENHARMONY_PIXEL_MAP];
const MAX_BYTES: number = 256 * 1024 * 1024;
const MAX_RECORDS: number = 16;

export class DroppedImage {
  name: string;
  bytes: ArrayBuffer;
  constructor(name: string, bytes: ArrayBuffer) { this.name = name; this.bytes = bytes; }
}

// The adapter owns its packer and cache; UDMF owns the delivered PixelMap.
export class NativeDropAdapter {
  private busy: boolean = false;
  private key: string = '';
  private cacheDir: string;
  private cancel: (key: string) => void;
  constructor(cacheDir: string, cancel: (key: string) => void) {
    this.cacheDir = cacheDir;
    this.cancel = cancel;
  }

  stop(): void {
    if (this.key.length > 0) {
      try { this.cancel(this.key); } catch (error) { console.info('Drop cancellation unavailable'); }
    }
  }

  async receive(event: DragEvent, isCurrent: () => boolean): Promise<Array<DroppedImage>> {
    event.dragBehavior = DragBehavior.COPY;
    event.setResult(DragResult.DRAG_FAILED);
    if (this.busy || !isCurrent()) { return []; }
    this.busy = true;
    let directory: string = '';
    try {
      directory = fileIo.mkdtempSync(this.cacheDir + '/photocraft-drop-XXXXXX');
      const data = await new Promise<unifiedDataChannel.UnifiedData | null>((resolve, reject) => {
        let finished = false;
        this.key = event.startDataLoading({
          destUri: fileUri.getUriFromPath(directory),
          fileConflictOptions: unifiedDataChannel.FileConflictOptions.OVERWRITE,
          progressIndicator: unifiedDataChannel.ProgressIndicator.DEFAULT,
          dataProgressListener: (progress: unifiedDataChannel.ProgressInfo, data: unifiedDataChannel.UnifiedData | null): void => {
            if (finished || progress.status === unifiedDataChannel.ListenerStatus.PROCESSING) { return; }
            finished = true;
            if (progress.status === unifiedDataChannel.ListenerStatus.FINISHED && data !== null) { resolve(data); }
            else if (progress.status === unifiedDataChannel.ListenerStatus.CANCELED) { resolve(null); }
            else { reject(new Error('无法取得拖放图片（状态 ' + progress.status + '）')); }
          }
        });
        // Acknowledge the COPY handoff while onDrop is valid. Import errors use the editor error UI.
        event.setResult(DragResult.DRAG_SUCCESSFUL);
      });
      this.key = '';
      if (!isCurrent() || data === null) { return []; }
      const records = data.getRecords();
      if (records.length > MAX_RECORDS) { throw new Error('一次最多拖入 16 张图片'); }
      let images: Array<DroppedImage> = [];
      let uris: Set<string> = new Set<string>();
      let total = 0;
      for (const record of records) {
        if (!isCurrent()) { return []; }
        const uri = this.recordUri(record);
        if (uri.length > 0 && uris.has(uri)) { continue; }
        const item = uri.length > 0 ? await this.readUri(uri, MAX_BYTES - total) :
          await this.readImage(record, MAX_BYTES - total);
        if (item === null) { continue; }
        if (uri.length > 0) { uris.add(uri); }
        total += item.bytes.byteLength;
        images.push(item);
      }
      if (!isCurrent()) { return []; }
      if (images.length === 0) { throw new Error('拖放内容中没有可导入的图片或文件'); }
      return images;
    } finally {
      this.key = '';
      if (directory.length > 0) {
        try { await fileIo.rmdir(directory); } catch (error) { console.error('Drop cache cleanup failed'); }
      }
      this.busy = false;
    }
  }

  private recordUri(record: unifiedDataChannel.UnifiedRecord): string {
    const types = record.getTypes();
    if (types.includes(UTD.FILE_URI)) {
      return (record.getEntry(UTD.FILE_URI) as uniformDataStruct.FileUri).oriUri;
    }
    // Legacy Image/File records carry their URI as a class property.
    if (record instanceof unifiedDataChannel.Image) { return record.imageUri; }
    if (record instanceof unifiedDataChannel.File) { return record.uri; }
    return '';
  }

  private async readUri(uri: string, remaining: number): Promise<DroppedImage> {
    // Do not fetch web URLs or plain text. UDMF supplies authorized local files.
    if (!uri.startsWith('file://')) { throw new Error('拖放文件必须是系统授权的本地文件 URI'); }
    const source = new fileUri.FileUri(uri);
    const file = await fileIo.open(uri, fileIo.OpenMode.READ_ONLY);
    let bytes: ArrayBuffer;
    try {
      const size = (await fileIo.stat(file.fd)).size;
      this.checkSize(size, remaining);
      bytes = new ArrayBuffer(size);
      const view = new Uint8Array(bytes);
      let offset = 0;
      while (offset < size) {
        const chunk = new ArrayBuffer(Math.min(1024 * 1024, size - offset));
        const read = await fileIo.read(file.fd, chunk);
        if (read <= 0) { throw new Error('拖放文件读取不完整'); }
        view.set(new Uint8Array(chunk, 0, read), offset);
        offset += read;
      }
    } finally { await fileIo.close(file); }
    // Never give Save a transient cache path it could overwrite.
    return new DroppedImage(source.name.length > 0 ? source.name : 'dropped-image', bytes);
  }

  private async readImage(record: unifiedDataChannel.UnifiedRecord, remaining: number): Promise<DroppedImage | null> {
    const types = record.getTypes();
    if (types.includes(UTD.OPENHARMONY_PIXEL_MAP)) {
      const pixels = (record.getEntry(UTD.OPENHARMONY_PIXEL_MAP) as uniformDataStruct.PixelMap).pixelMap;
      const info = await pixels.getImageInfo();
      this.checkSize(info.size.width * info.size.height * 4, remaining);
      const packer = image.createImagePacker();
      try {
        const bytes = await packer.packing(pixels, { format: 'image/png', quality: 100,
          bufferSize: Math.min(remaining, info.size.width * info.size.height * 4 + 1024 * 1024) });
        this.checkSize(bytes.byteLength, remaining);
        return new DroppedImage('dropped-image.png', bytes);
      } finally { await packer.release(); }
    }
    for (const type of types) {
      const descriptor = uniformTypeDescriptor.getTypeDescriptor(type);
      if (descriptor === null || !descriptor.belongsTo(UTD.IMAGE)) { continue; }
      const value = record.getEntry(type);
      if (value instanceof ArrayBuffer) {
        this.checkSize(value.byteLength, remaining);
        const extension = descriptor.filenameExtensions[0] ?? '.png';
        return new DroppedImage('dropped-image' + extension, value);
      }
    }
    return null;
  }

  private checkSize(size: number, remaining: number): void {
    if (!Number.isSafeInteger(size) || size <= 0 || size > remaining) {
      throw new Error('拖放图片为空或总大小超过 256 MiB');
    }
  }
}
