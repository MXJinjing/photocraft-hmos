import native from 'libphotocraft.so';
import { common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';
import { i18n } from '@kit.LocalizationKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { applySystemBars, LOADING_CHROME } from '../SystemBars';
import { setOpenListener, takeOpens } from '../openInbox';
import { allowConfirmedClose, setCloseRequest } from '../closeGuard';

class NativeConfig {
  files: string;
  scale: number;
  language: string;
  constructor(files: string, scale: number, language: string) {
    this.files = files; this.scale = scale; this.language = language;
  }
}
class NativeMessage { kind: string = ''; text: string = ''; }
class NativeInput {
  kind: string = '';
  action: number = 0;
  x: number = 0;
  y: number = 0;
  code: number = 0;
  text: string = '';
  ctrl: boolean = false;
  shift: boolean = false;
  alt: boolean = false;
}

@Entry
@Component
struct Index {
  @State private failure: string = '';
  private context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
  private generation: number = 0;
  private alive: boolean = false;
  private saveTargets: Map<string, string> = new Map();
  private drainOpens(): void {
    for (let file of takeOpens()) { native.open(file.name, file.bytes); }
  }
  aboutToAppear(): void {
    console.info('PhotoCraft native page appearing');
    this.alive = true;
    this.generation++;
    let scale: number = this.getUIContext().vp2px(1);
    native.init(JSON.stringify(new NativeConfig(this.context.filesDir, scale, i18n.System.getSystemLanguage())),
      (id: number, message: string, data: ArrayBuffer) => { this.receive(id, message, data); });
    setOpenListener(() => this.drainOpens());
    setCloseRequest(() => native.close());
    this.drainOpens();
    applySystemBars(this.context, LOADING_CHROME);
  }
  aboutToDisappear(): void {
    this.alive = false;
    this.generation++;
    this.saveTargets.clear();
    setOpenListener(null);
    setCloseRequest(null);
    native.active(false);
  }
  onBackPress(): boolean {
    let input = new NativeInput(); input.kind = 'key'; input.code = 2070;
    native.input(JSON.stringify(input)); input.action = 1; native.input(JSON.stringify(input));
    return true;
  }
  private receive(id: number, message: string, data: ArrayBuffer): void {
    let request = JSON.parse(message) as NativeMessage;
    console.info(`PhotoCraft native ${request.kind}: ${request.text}`);
    if (!this.alive) { if (id > 0) native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return; }
    if (request.kind === 'error') { this.failure = request.text; }
    else if (request.kind === 'ready') { this.failure = ''; }
    else if (request.kind === 'open') { this.openFile(id); }
    else if (request.kind === 'save_select') { this.selectSave(id, request.text); }
    else if (request.kind === 'write') { this.writeFile(id, request.text, data); }
    else if (request.kind === 'close') {
      allowConfirmedClose(); this.context.terminateSelf();
    } else if (request.kind === 'url') {
      this.context.startAbility({ action: 'ohos.want.action.viewData', uri: request.text })
        .catch((error: BusinessError) => { console.error(`External link: ${error.message}`); });
    }
  }
  private async openFile(id: number): Promise<void> {
    let generation = this.generation;
    try {
      let options = new picker.DocumentSelectOptions(); options.maxSelectNumber = 1;
      let uris = await new picker.DocumentViewPicker(this.context).select(options);
      if (!this.alive || generation !== this.generation || uris.length === 0) {
        native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return;
      }
      let file = await fileIo.open(uris[0], fileIo.OpenMode.READ_ONLY);
      let bytes: ArrayBuffer;
      try {
        let size = (await fileIo.stat(file.fd)).size;
        if (size <= 0 || size > 256 * 1024 * 1024) throw new Error('File is empty or exceeds 256 MiB');
        bytes = new ArrayBuffer(size);
        let view = new Uint8Array(bytes); let offset = 0;
        while (offset < size) {
          let chunk = new ArrayBuffer(Math.min(1024 * 1024, size - offset));
          let read = await fileIo.read(file.fd, chunk);
          if (read <= 0) throw new Error('Incomplete file read');
          view.set(new Uint8Array(chunk, 0, read), offset); offset += read;
        }
      } finally { await fileIo.close(file); }
      native.reply(id, true, new fileUri.FileUri(uris[0]).name, bytes);
    } catch (error) {
      let failure = error as Error;
      native.reply(id, false, failure.message, new ArrayBuffer(0));
    }
  }
  private async selectSave(id: number, suggested: string): Promise<void> {
    let generation = this.generation;
    try {
      let name = suggested.substring(suggested.lastIndexOf('/') + 1).replace(/[\\:*?"<>|]/g, '_');
      let options = new picker.DocumentSaveOptions(); options.newFileNames = [name];
      let dot = name.lastIndexOf('.');
      if (dot >= 0) options.fileSuffixChoices = ['PhotoCraft|' + name.substring(dot)];
      let uris = await new picker.DocumentViewPicker(this.context).save(options);
      if (!this.alive || generation !== this.generation || uris.length === 0) {
        native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return;
      }
      let selectedName = new fileUri.FileUri(uris[0]).name;
      if (selectedName.length === 0) throw new Error('Selected file has no name');
      this.saveTargets.set(selectedName, uris[0]);
      native.reply(id, true, selectedName, new ArrayBuffer(0));
    } catch (error) {
      let failure = error as Error;
      native.reply(id, false, failure.message, new ArrayBuffer(0));
    }
  }
  private async writeFile(id: number, name: string, bytes: ArrayBuffer): Promise<void> {
    try {
      let uri = this.saveTargets.get(name);
      this.saveTargets.delete(name);
      if (uri === undefined) throw new Error('No system save target for ' + name);
      let file = await fileIo.open(uri, fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.CREATE);
      try {
        await fileIo.truncate(file.fd, 0);
        let offset = 0;
        while (offset < bytes.byteLength) {
          let chunk = bytes.slice(offset, Math.min(offset + 1024 * 1024, bytes.byteLength));
          let written = await fileIo.write(file.fd, chunk);
          if (written <= 0) throw new Error('Incomplete file write');
          offset += written;
        }
        await fileIo.fsync(file.fd);
      } finally { await fileIo.close(file); }
      native.reply(id, true, name, new ArrayBuffer(0));
    } catch (error) {
      let failure = error as Error;
      native.reply(id, false, failure.message, new ArrayBuffer(0));
    }
  }
  build() {
    Stack() {
      XComponent({ id: 'photocraft-native', type: XComponentType.SURFACE, libraryname: 'photocraft' })
        .width('100%').height('100%')
        .focusable(true).defaultFocus(true)
        .onBlur(() => { let input = new NativeInput(); input.kind = 'blur'; native.input(JSON.stringify(input)); })
      if (this.failure.length > 0) {
        Column() {
          Text('PhotoCraft 原生运行错误').fontSize(20).fontColor('#FFFFFF')
          Text(this.failure).fontSize(14).fontColor('#FFFFFF').margin({ top: 12 })
          Button('关闭提示').onClick(() => { this.failure = ''; })
        }.padding(24).backgroundColor('#DD262626').borderRadius(8).width('80%')
      }
    }.width('100%').height('100%').backgroundColor(LOADING_CHROME)
  }
}
