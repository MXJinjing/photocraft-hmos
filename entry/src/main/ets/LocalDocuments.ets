import { fileIo } from '@kit.CoreFileKit';
import { BusinessError } from '@kit.BasicServicesKit';

// Public DOWNLOAD paths must go through CoreFileKit's document permission bridge.
export class LocalDocuments {
  private sequence: number = 0;
  private directory: string;
  private staging: string;
  constructor(directory: string, staging: string) { this.directory = directory; this.staging = staging; }
  owns(path: string): boolean {
    // 普通保存只接受应用文档目录的直接子文件，不把外部路径或子目录当成本地文档。
    let prefix = this.directory + '/';
    return path.startsWith(prefix) && path.substring(prefix.length).length > 0 &&
      !path.substring(prefix.length).includes('/') && !path.substring(prefix.length).includes('\\');
  }
  async available(name: string): Promise<string> {
    // 将递增编号插入扩展名前；只有“文件不存在”才表示名称可用，权限错误继续上抛。
    let base = name.substring(name.lastIndexOf('/') + 1).replace(/[\\:*?"<>|\x00-\x1F\x7F]/g, '_').trim();
    if (base.length === 0 || base === '.' || base === '..') { throw new Error('文件名不能为空'); }
    let dot = base.lastIndexOf('.');
    for (let suffix = 0; suffix < 10000; suffix++) {
      let numbered = suffix === 0 ? base : dot >= 0 ?
        base.substring(0, dot) + suffix.toString() + base.substring(dot) : base + suffix.toString();
      let path = this.directory + '/' + numbered;
      try {
        let existing = await fileIo.open(path, fileIo.OpenMode.READ_ONLY);
        await fileIo.close(existing);
      } catch (error) {
        if ((error as BusinessError).code === 13900002) { return path; }
        throw new Error((error as Error).message);
      }
    }
    throw new Error('同名文件过多');
  }
  async write(path: string, bytes: ArrayBuffer): Promise<void> {
    if (!this.owns(path)) { throw new Error('Invalid application document path'); }
    // Public document directories allow opening files, but not rename/unlink.
    // Stage the completed export in the sandbox and keep a backup while publishing
    // through a granted file descriptor. Never fall back to a location picker.
    let token = Date.now().toString() + '-' + (++this.sequence).toString();
    let temp = this.staging + '/photocraft-save-' + token + '.tmp';
    let backup = this.staging + '/photocraft-backup-' + token + '.psd';
    let keepBackup: boolean = false;
    try {
      let staged = await fileIo.open(temp, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE | fileIo.OpenMode.TRUNC);
      try {
        let offset = 0;
        while (offset < bytes.byteLength) {
          let chunk = bytes.slice(offset, Math.min(offset + 1024 * 1024, bytes.byteLength));
          let written = await fileIo.write(staged.fd, chunk);
          if (written <= 0) { throw new Error('Incomplete file write'); }
          offset += written;
        }
        await fileIo.fsync(staged.fd);
      } finally { await fileIo.close(staged); }
      let target = await fileIo.open(path, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE);
      try {
        await fileIo.copyFile(target.fd, backup);
        try {
          await fileIo.copyFile(temp, target.fd);
          await fileIo.fsync(target.fd);
        } catch (error) {
          try {
            await fileIo.copyFile(backup, target.fd);
            await fileIo.fsync(target.fd);
          } catch (restoreError) {
            keepBackup = true;
            throw new Error((error as Error).message + '; recovery copy: ' + backup);
          }
          throw new Error((error as Error).message);
        }
      } finally { await fileIo.close(target); }
    } finally {
      try {
        if (await fileIo.access(temp)) { await fileIo.unlink(temp); }
        if (!keepBackup && await fileIo.access(backup)) { await fileIo.unlink(backup); }
      } catch (error) { console.warn('PhotoCraft temporary file cleanup: ' + (error as Error).message); }
    }
  }
}
