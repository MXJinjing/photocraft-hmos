import { common } from '@kit.AbilityKit';
import { print } from '@kit.BasicServicesKit';
import { fileIo, fileUri } from '@kit.CoreFileKit';

class PrintMetadata {
  name: string = 'PhotoCraft';
  copies: number = 1;
  printer: string = '';
}

// Keep the sandbox PDF until the asynchronous job ends: opening preview is
// not completion, and a blocked job may still need its file later.
export class NativePrintAdapter {
  private sequence: number = 0;
  private disposed: boolean = false;
  private tasks: Map<string, print.PrintTask> = new Map();
  private context: common.UIAbilityContext;
  private report: (message: string) => void;
  constructor(context: common.UIAbilityContext, report: (message: string) => void) {
    this.context = context; this.report = report;
  }
  stop(): void { this.disposed = true; }
  start(): void { this.disposed = false; }

  async open(metadata: string, bytes: ArrayBuffer): Promise<void> {
    if (this.disposed) { throw new Error('cancelled'); }
    if (this.tasks.size >= 4) { throw new Error('请先完成或取消已有打印任务'); }
    let job = JSON.parse(metadata) as PrintMetadata;
    if (job.printer !== undefined && job.printer.length > 0) { throw new Error('请在系统打印面板中选择打印机'); }
    if (job.copies !== 1) { throw new Error('请在系统打印面板中设置份数'); }
    if (bytes.byteLength < 5 || bytes.byteLength > 256 * 1024 * 1024) {
      throw new Error('打印文件为空或超过 256 MiB');
    }
    let header = new Uint8Array(bytes, 0, 5);
    if (header[0] !== 37 || header[1] !== 80 || header[2] !== 68 || header[3] !== 70 || header[4] !== 45) {
      throw new Error('打印文件不是 PDF');
    }
    let name = job.name.replace(/[\\/:*?"<>|]/g, '_').substring(0, 80) || 'PhotoCraft';
    let path = `${this.context.cacheDir}/${name}-${Date.now()}-${this.sequence++}.pdf`;
    try {
      let file = await fileIo.open(path, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE | fileIo.OpenMode.TRUNC);
      try {
        let offset = 0;
        while (offset < bytes.byteLength) {
          let chunk = bytes.slice(offset, Math.min(offset + 1024 * 1024, bytes.byteLength));
          let written = await fileIo.write(file.fd, chunk);
          if (written <= 0) { throw new Error('打印文件写入不完整'); }
          offset += written;
        }
        await fileIo.fsync(file.fd);
      } finally { await fileIo.close(file); }
      if (this.disposed) { throw new Error('cancelled'); }
      let task = await print.print([fileUri.getUriFromPath(path)], this.context);
      this.tasks.set(path, task);
      // A listener failure must not delete a PDF already handed to the service.
      try {
        task.on('succeed', (): void => { this.finish(path); });
        task.on('cancel', (): void => { this.finish(path); });
        task.on('fail', (): void => { this.report('系统打印任务失败，请检查打印机连接和驱动'); this.finish(path); });
        task.on('block', (): void => { this.report('打印任务已阻塞，请在系统打印面板检查纸张、墨水和连接'); });
      } catch (error) {
        this.report(`系统打印预览已打开，但无法监听任务状态：${(error as Error).message}`);
      }
    } catch (error) {
      this.finish(path);
      throw new Error(`无法打开系统打印：${(error as Error).message}`);
    }
  }

  private finish(path: string): void {
    let task = this.tasks.get(path);
    this.tasks.delete(path);
    if (task !== undefined) {
      try { task.off('succeed'); task.off('cancel'); task.off('fail'); task.off('block'); }
      catch (error) { console.error(`Print listener cleanup: ${(error as Error).message}`); }
    }
    fileIo.unlink(path).catch((): void => {});
  }
}
