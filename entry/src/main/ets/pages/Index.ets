import native from 'libphotocraft.so';
import { documentDirectory } from '../documentDirectory';
import { LocalSaveDialog } from '../LocalSaveDialog';
import { LocalDocuments } from '../LocalDocuments';
import { saveFormatChoices } from '../saveFormats';
import { NativePrintAdapter } from '../NativePrintAdapter';
import { common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';
import { i18n } from '@kit.LocalizationKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { KeyboardAvoidMode } from '@kit.ArkUI';
import { applySystemBars, LOADING_CHROME, normalizeChrome } from '../SystemBars';
import { setOpenListener, takeOpens } from '../openInbox';
import { allowConfirmedClose, setCloseRequest } from '../closeGuard';
import { HarmonyStylusAdapter } from '../stylus/HarmonyStylusAdapter';
import { StylusPacket } from '../stylus/StylusProtocol';
import { NativeImeAdapter, NativeImeState } from '../input/NativeImeAdapter';
import { NativeCursorAdapter } from '../input/NativeCursorAdapter';
import { moveWindow, toggleWindowMaximize } from '../WindowChrome';
import { window } from '@kit.ArkUI';

class NativeConfig {
  files: string;
  documents: string;
  scale: number;
  language: string;
  constructor(files: string, documents: string, scale: number, language: string) {
    this.files = files; this.documents = documents; this.scale = scale; this.language = language;
  }
}
class NativeMessage { kind: string = ''; text: string = ''; }
class NativeStylusInput {
  kind: string = 'stylus';
  packet: StylusPacket;
  constructor(packet: StylusPacket) { this.packet = packet; }
}
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
  dx: number = 0;
  dy: number = 0;
}

@Entry
@Component
struct Index {
  @StorageLink('photocraftChrome') @Watch('onChromeChanged') private chrome: string = LOADING_CHROME;
  @State private failure: string = '';
  @State private initialized: boolean = false;
  @StorageLink('photocraftTitleRight') @Watch('onTitlebarChanged') private titleRight: number = 0;
  @StorageLink('photocraftTitleHeight') @Watch('onTitlebarChanged') private titleHeight: number = 0;
  private context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
  private generation: number = 0;
  // 页面离开或重新初始化后，旧的异步结果不能继续操作当前页面。
  private alive: boolean = false;
  private localSaveId: number = 0;
  private documents: LocalDocuments | undefined = undefined;
  private localSaveSuggested: string = '';
  private localSaveDialog: CustomDialogController = new CustomDialogController({
    builder: LocalSaveDialog({ initialName: this.localSaveSuggested,
      onConfirm: (name: string): void => { this.finishLocalSave(name); },
      onCancel: (): void => { this.finishLocalSave(''); } }),
    autoCancel: false,
    cancel: (): void => { this.finishLocalSave(''); }
  });
  // Save As 返回给 Rust 的逻辑标识映射到系统授权 URI，写入后消费一次。
  private saveTargets: Map<string, string> = new Map();
  private stylus: HarmonyStylusAdapter = new HarmonyStylusAdapter((packet: StylusPacket): void => {
    if (this.alive) { native.input(JSON.stringify(new NativeStylusInput(packet))); }
  });
  private cursor: NativeCursorAdapter = new NativeCursorAdapter(this.getUIContext());
  private ime: NativeImeAdapter = new NativeImeAdapter((message: string): void => native.input(message));
  private printing: NativePrintAdapter = new NativePrintAdapter(this.context,
    (message: string): void => { if (this.alive) { this.failure = message; } });
  private drainOpens(): void {
    for (let file of takeOpens()) { native.open(file.name, file.bytes); }
  }
  aboutToAppear(): void {
    console.info('PhotoCraft native page appearing');
    this.alive = true;
    this.printing.start();
    this.generation++;
    this.getUIContext().setKeyboardAvoidMode(KeyboardAvoidMode.RESIZE);
    this.initializeHost(this.generation);
    applySystemBars(this.context, this.chrome);
  }
  private async initializeHost(generation: number): Promise<void> {
    // 先取得文档目录再创建原生宿主，避免首次保存时目录尚未准备好。
    try {
      let documents = await documentDirectory(this.context);
      if (!this.alive || generation !== this.generation) { return; }
      this.documents = new LocalDocuments(documents, this.context.filesDir);
      let scale: number = this.getUIContext().vp2px(1);
      native.init(JSON.stringify(new NativeConfig(this.context.filesDir, documents, scale, i18n.System.getSystemLanguage())),
        (id: number, message: string, data: ArrayBuffer) => { this.receive(id, message, data); });
      this.initialized = true;
      this.onTitlebarChanged();
      this.failure = '';
      setOpenListener(() => this.drainOpens());
      setCloseRequest(() => native.close());
      this.drainOpens();
      this.stylus.start();
    } catch (error) {
      if (this.alive && generation === this.generation) { this.failure = (error as Error).message; }
    }
  }
  aboutToDisappear(): void {
    this.alive = false;
    this.printing.stop();
    this.generation++;
    this.localSaveDialog.close();
    this.finishLocalSave('');
    this.initialized = false;
    this.saveTargets.clear();
    this.stylus.stop();
    this.ime.stop();
    this.cursor.reset();
    setOpenListener(null);
    setCloseRequest(null);
    native.active(false);
  }
  private onTitlebarChanged(): void {
    // ArkUI 的布局值使用 vp；桥接协议统一传物理像素，由 Rust 按 egui 缩放换算。
    if (!this.initialized) { return; }
    let input = new NativeInput();
    input.kind = 'titlebar';
    input.dx = this.getUIContext().vp2px(this.titleRight);
    input.dy = this.getUIContext().vp2px(this.titleHeight);
    native.input(JSON.stringify(input));
  }
  onBackPress(): boolean {
    let input = new NativeInput(); input.kind = 'key'; input.code = 2070;
    native.input(JSON.stringify(input)); input.action = 1; native.input(JSON.stringify(input));
    return true;
  }
  onChromeChanged(): void {
    if (this.alive) { applySystemBars(this.context, this.chrome); }
  }
  private receive(id: number, message: string, data: ArrayBuffer): void {
    // id > 0 是需要 reply 的平台服务请求；id = 0 是无需应答的状态通知。
    let request = JSON.parse(message) as NativeMessage;
    if (request.kind !== 'ime') { console.info(`PhotoCraft native ${request.kind}: ${request.text}`); }
    if (!this.alive) { if (id > 0) native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return; }
    if (request.kind === 'error') { this.failure = request.text; }
    else if (request.kind === 'ready') { this.failure = ''; this.onTitlebarChanged(); }
    else if (request.kind === 'window_move' || request.kind === 'window_maximize') {
      window.getLastWindow(this.context).then((win: window.Window) => {
        return request.kind === 'window_move' ? moveWindow(win) : toggleWindowMaximize(win);
      }).catch((error: BusinessError) => { console.info(`Window action: ${error.code} ${error.message}`); });
    }
    else if (request.kind === 'cursor') { this.cursor.update(request.text); }
    else if (request.kind === 'ime') { this.ime.update(JSON.parse(request.text) as NativeImeState); }
    else if (request.kind === 'chrome') {
      let color: string = normalizeChrome(request.text);
      if (color.length > 0 && color !== this.chrome) {
        AppStorage.setOrCreate('photocraftChrome', color);
      }
    }
    else if (request.kind === 'open') { this.openFile(id); }
    else if (request.kind === 'local_save') {
      this.ime.stop(); this.cursor.reset();
      this.localSaveId = id; this.localSaveSuggested = request.text; this.localSaveDialog.open();
    }
    else if (request.kind === 'save_select') { this.selectSave(id, request.text); }
    else if (request.kind === 'write') { this.writeFile(id, request.text, data); }
    else if (request.kind === 'print') { this.printFile(id, request.text, data); }
    else if (request.kind === 'close') {
      allowConfirmedClose(); this.context.terminateSelf();
    } else if (request.kind === 'url') {
      this.context.startAbility({ action: 'ohos.want.action.viewData', uri: request.text })
        .catch((error: BusinessError) => { console.error(`External link: ${error.message}`); });
    }
  }
  private async printFile(id: number, metadata: string, bytes: ArrayBuffer): Promise<void> {
    this.ime.stop(); this.cursor.reset();
    try {
      await this.printing.open(metadata, bytes);
      native.reply(id, true, 'HarmonyOS', new ArrayBuffer(0));
    } catch (error) {
      native.reply(id, false, (error as Error).message, new ArrayBuffer(0));
    } finally { this.restoreEditorFocus(); }
  }
  private async finishLocalSave(name: string): Promise<void> {
    let id = this.localSaveId;
    this.localSaveId = 0;
    if (id > 0) {
      try {
        if (name.length === 0 || this.documents === undefined) {
          native.reply(id, false, 'cancelled', new ArrayBuffer(0));
        } else {
          let path = await this.documents.available(name);
          native.reply(id, true, path, new ArrayBuffer(0));
        }
      } catch (error) {
        native.reply(id, false, (error as Error).message, new ArrayBuffer(0));
      } finally { this.restoreEditorFocus(); }
    }
  }
  private restoreEditorFocus(): void {
    // 系统选择器或弹窗关闭后恢复 XComponent 焦点，硬件快捷键才能继续到达编辑器。
    if (this.alive && this.initialized) {
      this.getUIContext().getFocusController().requestFocus('photocraft-editor');
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
    } finally {
      this.restoreEditorFocus();
    }
  }
  private async selectSave(id: number, suggested: string): Promise<void> {
    let generation = this.generation;
    try {
      let name = suggested.substring(suggested.lastIndexOf('/') + 1).replace(/[\\:*?"<>|]/g, '_');
      let options = new picker.DocumentSaveOptions(); options.newFileNames = [name];
      options.fileSuffixChoices = saveFormatChoices(name);
      let uris = await new picker.DocumentViewPicker(this.context).save(options);
      if (!this.alive || generation !== this.generation || uris.length === 0) {
        native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return;
      }
      let selectedName = new fileUri.FileUri(uris[0]).name;
      if (selectedName.length === 0) throw new Error('Selected file has no name');
      let target = 'save-as/' + id.toString() + '/' + selectedName;
      this.saveTargets.set(target, uris[0]);
      native.reply(id, true, target, new ArrayBuffer(0));
    } catch (error) {
      let failure = error as Error;
      native.reply(id, false, failure.message, new ArrayBuffer(0));
    } finally {
      this.restoreEditorFocus();
    }
  }
  private async writeFile(id: number, name: string, bytes: ArrayBuffer): Promise<void> {
    try {
      if (this.documents !== undefined && this.documents.owns(name)) {
        await this.documents.write(name, bytes);
        native.reply(id, true, name, new ArrayBuffer(0));
        return;
      }
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
      if (this.initialized) {
      XComponent({ id: 'photocraft-native', type: XComponentType.SURFACE, libraryname: 'photocraft' })
        .width('100%').height('100%')
        .onLoad(() => { this.restoreEditorFocus(); })
        .onAreaChange((_oldArea: Area, area: Area): void => {
          this.ime.setOrigin(this.getUIContext().vp2px(Number(area.globalPosition.x)),
            this.getUIContext().vp2px(Number(area.globalPosition.y)));
        })
        .id('photocraft-editor').focusable(true).defaultFocus(true).focusOnTouch(true)
        .onBlur(() => { this.cursor.reset(); let input = new NativeInput(); input.kind = 'blur'; native.input(JSON.stringify(input)); })
      } else {
        Text('正在准备文档目录…').fontSize(18).fontColor('#FFFFFF')
      }
      if (this.failure.length > 0) {
        Column() {
          Text('PhotoCraft 原生运行错误').fontSize(20).fontColor('#FFFFFF')
          Text(this.failure).fontSize(14).fontColor('#FFFFFF').margin({ top: 12 })
          if (!this.initialized) {
            Button('重试').onClick(() => { this.failure = ''; this.initializeHost(this.generation); })
          } else {
            Button('关闭提示').onClick(() => { this.failure = ''; })
          }
        }.padding(24).backgroundColor('#DD262626').borderRadius(8).width('80%')
      }
    }.width('100%').height('100%').backgroundColor(this.chrome)
  }
}
