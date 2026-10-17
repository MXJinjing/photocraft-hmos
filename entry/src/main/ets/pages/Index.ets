import native from 'libphotocraft.so';
import { applicationDocumentUri } from '../documentDirectory';
import { LocalDocuments } from '../LocalDocuments';
import { savePickerOptions, SaveTargets } from '../savePicker';
import { NativePrintAdapter } from '../NativePrintAdapter';
import { abilityAccessCtrl, common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';
import { i18n } from '@kit.LocalizationKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { KeyboardAvoidMode } from '@kit.ArkUI';
import { applySystemBars, LOADING_CHROME, normalizeChrome } from '../SystemBars';
import { NavigationIndicatorInset, SafeInsets } from '../NavigationInset';
import { IMMERSIVE_KEY } from '../Immersive';
import { setOpenListener, takeOpens } from '../openInbox';
import { allowConfirmedClose, setCloseRequest } from '../closeGuard';
import { HarmonyStylusAdapter } from '../stylus/HarmonyStylusAdapter';
import { StylusPacket } from '../stylus/StylusProtocol';
import { NativeImeAdapter, NativeImeState } from '../input/NativeImeAdapter';
import { NativeCursorAdapter } from '../input/NativeCursorAdapter';
import { IMAGE_DROP_TYPES, NativeDropAdapter } from '../input/NativeDropAdapter';
import { DropPreview, DropTargetFeedback, localDropPoint } from '../input/DropPreview';
import { isFiniteHoverPoint, penHoverLeaveMessage, penHoverMoveMessage } from '../input/PenHoverPreview';
import { moveWindow, toggleWindowMaximize } from '../WindowChrome';
import { window } from '@kit.ArkUI';
import { configureDisplayColor } from '../DisplayColor';
import { SYSTEM_THEME_DARK, SYSTEM_THEME_LIGHT } from '../SystemTheme';

// Set to true and rebuild to show the measured safe-area numbers on screen (also in hilog).
const INSET_DEBUG: boolean = false;

class NativeConfig {
  files: string;
  documents: string;
  scale: number;
  language: string;
  displayP3: boolean;
  systemTheme: string;
  constructor(files: string, documents: string, scale: number, language: string, displayP3: boolean, systemTheme: string) {
    this.files = files; this.documents = documents; this.scale = scale; this.language = language;
    this.displayP3 = displayP3;
    this.systemTheme = systemTheme;
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
  @State private dropPreview: DropPreview = new DropPreview();
  private editorOriginX: number = 0;
  private editorOriginY: number = 0;
  private lastInsetDiag: string = '';
  @StorageLink('photocraftTitleRight') @Watch('onTitlebarChanged') private titleRight: number = 0;
  @StorageLink('photocraftTitleHeight') @Watch('onTitlebarChanged') private titleHeight: number = 0;
  // 窗口为沉浸式布局时页面覆盖状态栏与底部导航条（小横条）；两处被覆盖的高度以物理像素交给原生编辑器避让。
  @StorageLink('photocraftImmersive') @Watch('onImmersiveChanged') private immersive: boolean = false;
  // 调试诊断：把 INSET_DEBUG 改为 true 重新构建即可在左上角显示实测数值；默认关闭。
  @StorageLink('photocraftInsetDiag') private insetDiag: string = '';
  private bottomInset: NavigationIndicatorInset = new NavigationIndicatorInset((insets: SafeInsets): void => {
    this.sendSafeInset(insets);
  }, (text: string): void => { this.reportInset(text); });
  private context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
  private generation: number = 0;
  // 页面离开或重新初始化后，旧的异步结果不能继续操作当前页面。
  private alive: boolean = false;
  private documents: LocalDocuments | undefined = undefined;
  private documentUri: string = '';
  // Ordinary Save retains its authorized URI; Save As consumes its URI once.
  private saveTargets: SaveTargets = new SaveTargets();
  private stylus: HarmonyStylusAdapter = new HarmonyStylusAdapter((packet: StylusPacket): void => {
    if (this.alive) { native.input(JSON.stringify(new NativeStylusInput(packet))); }
  }, async (): Promise<boolean> => {
    let result = await abilityAccessCtrl.createAtManager().requestPermissionsFromUser(this.context, ['ohos.permission.ACCESS_BLUETOOTH']);
    return result.authResults.length > 0 && result.authResults[0] === 0;
  });
  private cursor: NativeCursorAdapter = new NativeCursorAdapter(this.getUIContext());
  private dropping: NativeDropAdapter = new NativeDropAdapter(this.context.cacheDir,
    (key: string): void => { this.getUIContext().getDragController().cancelDataLoading(key); },
    (name: string, fd: number): void => { native.openFile(name, fd); });
  private ime: NativeImeAdapter = new NativeImeAdapter((message: string): void => native.input(message));
  private printing: NativePrintAdapter = new NativePrintAdapter(this.context,
    (message: string): void => { if (this.alive) { this.failure = message; } });
  private drainOpens(): void {
    for (let file of takeOpens()) {
      let name = file.name;
      try {
        let path = file.uri.length > 0 ? new fileUri.FileUri(file.uri).path : '';
        if (this.documents?.owns(path)) name = path;
      } catch (error) { console.info('Open source path unavailable'); }
      try {
        native.openFile(name, file.file.fd);
      } catch (error) {
        this.failure = (error as Error).message;
      } finally {
        try { fileIo.closeSync(file.file); }
        catch (error) { console.error(`Open close failed: ${(error as Error).message}`); }
      }
    }
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
      let uri = await applicationDocumentUri(this.context);
      let documents = new fileUri.FileUri(uri).path;
      if (documents.length === 0) throw new Error('Application document directory has no path');
      if (!this.alive || generation !== this.generation) { return; }
      this.documents = new LocalDocuments(documents, this.context.filesDir);
      this.documentUri = uri;
      let scale: number = this.getUIContext().vp2px(1);
      const displayP3 = await configureDisplayColor(this.context);
      if (!this.alive || generation !== this.generation) { return; }
      let systemTheme = AppStorage.get<string>('photocraftSystemTheme') ?? '';
      if (systemTheme !== SYSTEM_THEME_DARK && systemTheme !== SYSTEM_THEME_LIGHT) {
        systemTheme = '';
      }
      native.init(JSON.stringify(new NativeConfig(this.context.filesDir, documents, scale, i18n.System.getSystemLanguage(), displayP3, systemTheme)),
        (id: number, message: string, data: ArrayBuffer) => { this.receive(id, message, data); });
      this.initialized = true;
      this.onTitlebarChanged();
      this.startBottomInset(generation);
      this.failure = '';
      setOpenListener(() => this.drainOpens());
      setCloseRequest((): void => { native.close(); });
      this.drainOpens();
      this.stylus.start();
    } catch (error) {
      if (this.alive && generation === this.generation) { this.failure = (error as Error).message; }
    }
  }
  aboutToDisappear(): void {
    this.alive = false;
    this.printing.stop();
    this.dropping.stop();
    this.dropPreview.hide();
    this.generation++;
    this.initialized = false;
    this.saveTargets.clear();
    this.stylus.stop();
    this.ime.stop();
    this.cursor.reset();
    this.bottomInset.stop();
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
  private startBottomInset(generation: number): void {
    window.getLastWindow(this.context).then((win: window.Window) => {
      if (this.alive && generation === this.generation) {
        this.bottomInset.setImmersive(this.immersive);
        this.bottomInset.start(win);
      }
    }).catch((error: BusinessError) => {
      console.info(`Navigation indicator inset unavailable: ${error.code} ${error.message}`);
    });
  }
  private onImmersiveChanged(): void { this.bottomInset.setImmersive(this.immersive); }
  private reportInset(text: string): void {
    if (text !== this.lastInsetDiag) {
      this.lastInsetDiag = text;
      console.info(`PhotoCraft bottom inset: ${text}`);
      if (INSET_DEBUG) { AppStorage.setOrCreate('photocraftInsetDiag', text); }
    }
  }
  private sendSafeInset(insets: SafeInsets): void {
    // 协议统一使用物理像素：dx=顶部（状态栏）遮挡，dy=底部（导航条）遮挡；Rust 只做单位换算。
    if (!this.alive || !this.initialized) { return; }
    let input = new NativeInput();
    input.kind = 'safe_inset';
    input.dx = insets.top;
    input.dy = insets.bottom;
    native.input(JSON.stringify(input));
    console.info(`PhotoCraft bottom inset sent: top=${insets.top} bottom=${insets.bottom}`);
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
    else if (request.kind === 'ready') { this.failure = ''; this.onTitlebarChanged(); this.bottomInset.resend(); }
    else if (request.kind === 'window_move' || request.kind === 'window_maximize') {
      window.getLastWindow(this.context).then((win: window.Window) => {
        return request.kind === 'window_move' ? moveWindow(win) : toggleWindowMaximize(win);
      }).catch((error: BusinessError) => { console.info(`Window action: ${error.code} ${error.message}`); });
    }
    else if (request.kind === 'drop_target') {
      this.dropPreview.apply(JSON.parse(request.text) as DropTargetFeedback, this.getUIContext().vp2px(1));
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
    else if (request.kind === 'save_default') { this.selectSave(id, request.text, true); }
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
      try {
        if (!this.alive || generation !== this.generation) {
          native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return;
        }
        let opened = new fileUri.FileUri(uris[0]);
        native.replyFile(id, this.documents?.owns(opened.path) ? opened.path : opened.name, file.fd);
      } finally { await fileIo.close(file); }
    } catch (error) {
      let failure = error as Error;
      native.reply(id, false, failure.message, new ArrayBuffer(0));
    } finally {
      this.restoreEditorFocus();
    }
  }
  private async selectSave(id: number, suggested: string, ordinary: boolean = false): Promise<void> {
    this.ime.stop(); this.cursor.reset();
    let generation = this.generation;
    try {
      let options = savePickerOptions(suggested, this.documentUri);
      let uris = await new picker.DocumentViewPicker(this.context).save(options);
      if (!this.alive || generation !== this.generation || uris.length === 0) {
        native.reply(id, false, 'cancelled', new ArrayBuffer(0)); return;
      }
      let selectedName = new fileUri.FileUri(uris[0]).name;
      if (selectedName.length === 0) throw new Error('Selected file has no name');
      let target = this.saveTargets.select(id, selectedName, uris[0], ordinary);
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
      let uri = this.saveTargets.take(name);
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
  private previewDrop(event: DragEvent): void {
    if (!this.alive || !this.initialized) { return; }
    const point = localDropPoint(event.getWindowX(), event.getWindowY(), this.editorOriginX, this.editorOriginY);
    if (point === null) { this.dropPreview.hide(); return; }
    event.dragBehavior = DragBehavior.COPY;
    event.setResult(DragResult.DROP_ENABLED);
    const input = new NativeInput();
    input.kind = 'drop_hover';
    input.code = this.dropPreview.move();
    input.x = this.getUIContext().vp2px(point.x);
    input.y = this.getUIContext().vp2px(point.y);
    native.input(JSON.stringify(input));
  }
  private async dropImages(event: DragEvent): Promise<void> {
    // Capture before async loading: DragEvent methods are valid only during the callback.
    const point = localDropPoint(event.getWindowX(), event.getWindowY(), this.editorOriginX, this.editorOriginY);
    this.dropPreview.hide();
    if (point === null) { event.setResult(DragResult.DRAG_FAILED); return; }
    const x = this.getUIContext().vp2px(point.x);
    const y = this.getUIContext().vp2px(point.y);
    const generation = this.generation;
    const isCurrent = (): boolean => this.alive && this.initialized && generation === this.generation;
    try {
      const images = await this.dropping.receive(event, isCurrent, (name: string, fd: number): void => {
        if (isCurrent()) {
          native.dropFile(name, fd, x, y);
          console.info('PhotoCraft drop queued: ' + name);
        }
      });
      if (!isCurrent()) { return; }
      for (const image of images) {
        native.drop(image.name, image.bytes, x, y);
        console.info('PhotoCraft drop queued: ' + image.name);
      }
    } catch (error) {
      if (isCurrent()) { this.failure = (error as Error).message; }
    } finally {
      if (isCurrent()) { this.restoreEditorFocus(); }
    }
  }
  private sendPenHoverMove(event: HoverEvent): void {
    if (!this.alive || !this.initialized) { return; }
    let x = Number(event.x);
    let y = Number(event.y);
    if (!isFiniteHoverPoint(x, y)) { return; }
    // onHoverMove reports component-local vp; the native bridge expects physical pixels.
    let xPx = this.getUIContext().vp2px(x);
    let yPx = this.getUIContext().vp2px(y);
    native.input(penHoverMoveMessage(xPx, yPx));
  }
  private sendPenHoverLeave(isHover: boolean): void {
    // Hide on leave; actual hover motion resumes preview without an entry gate.
    if (isHover || !this.alive || !this.initialized) { return; }
    native.input(penHoverLeaveMessage());
  }
  build() {
    Stack() {
      if (this.initialized) {
      XComponent({ id: 'photocraft-native', type: XComponentType.SURFACE, libraryname: 'photocraft' })
        .width('100%').height('100%')
        .onLoad(() => { this.restoreEditorFocus(); })
        .onAreaChange((_oldArea: Area, area: Area): void => {
          this.editorOriginX = Number(area.globalPosition.x);
          this.editorOriginY = Number(area.globalPosition.y);
          // 布局框位置/高度用于判断系统区域是否已被避让（键盘 RESIZE 会缩小它）。
          this.bottomInset.setContentBounds(
            this.getUIContext().vp2px(this.editorOriginY),
            this.getUIContext().vp2px(Number(area.height)));
          this.ime.setOrigin(this.getUIContext().vp2px(this.editorOriginX),
            this.getUIContext().vp2px(this.editorOriginY));
        })
        .allowDrop(IMAGE_DROP_TYPES)
        .onDragEnter((event: DragEvent): void => { this.previewDrop(event); })
        .onDragMove((event: DragEvent): void => { this.previewDrop(event); })
        .onDragLeave((): void => { this.dropPreview.hide(); })
        // Required by startDataLoading: avoid the default prefetch consuming UDMF data.
        .onDrop((event: DragEvent): void => { this.dropImages(event); }, { disableDataPrefetch: true })
        .onHoverMove((event: HoverEvent): void => { this.sendPenHoverMove(event); })
        .onHover((isHover: boolean): void => { this.sendPenHoverLeave(isHover); })
        .id('photocraft-editor').focusable(true).defaultFocus(true).focusOnTouch(true)
        .onBlur(() => { this.cursor.reset(); let input = new NativeInput(); input.kind = 'blur'; native.input(JSON.stringify(input)); })
      } else {
        Text('正在准备文档目录…').fontSize(18).fontColor('#FFFFFF')
      }
      if (this.dropPreview.visible && this.dropPreview.mode === 'place') {
        Row() {}.width(this.dropPreview.width).height(this.dropPreview.height)
          .position({ x: this.dropPreview.left, y: this.dropPreview.top })
          .backgroundColor('#142D64F5').border({ width: 2, color: '#2D64F5' })
          .hitTestBehavior(HitTestMode.None)
      }
      if (this.failure.length > 0) {
        Column() {
          Text('PhotoCraft For HarmonyOS 原生运行错误').fontSize(20).fontColor('#FFFFFF')
          Text(this.failure).fontSize(14).fontColor('#FFFFFF').margin({ top: 12 })
          if (!this.initialized) {
            Button('重试').onClick(() => { this.failure = ''; this.initializeHost(this.generation); })
          } else {
            Button('关闭提示').onClick(() => { this.failure = ''; })
          }
        }.padding(24).backgroundColor('#DD262626').borderRadius(8).width('80%')
      }
      if (INSET_DEBUG && this.insetDiag.length > 0) {
        Text(this.insetDiag).fontSize(11).fontColor('#FFFFFF').backgroundColor('#99000000')
          .padding(4).width('70%')
          .position({ x: 8, y: 96 }).hitTestBehavior(HitTestMode.None)
      }
    }.width('100%').height('100%').backgroundColor(this.chrome)
  }
}
