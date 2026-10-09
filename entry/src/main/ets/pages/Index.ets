import { common } from '@kit.AbilityKit';
import { font } from '@kit.ArkUI';
import { fileIo, picker } from '@kit.CoreFileKit';
import { webview } from '@kit.ArkWeb';
import { BusinessError } from '@kit.BasicServicesKit';
import { i18n } from '@kit.LocalizationKit';
import { HarmonyStylusAdapter } from '../stylus/HarmonyStylusAdapter';
import { StylusAdapter, StylusPacket, stylusPacketScript } from '../stylus/StylusProtocol';
import { setOpenListener, takeOpens } from '../openInbox';
import { applySystemBars, normalizeChrome } from '../SystemBars';
import { allowConfirmedClose, restoreCloseGuard, setCloseRequest } from '../closeGuard';

class PhotocraftHost {
  context: common.UIAbilityContext | null = null;
  // True when a download or the system save picker is still open. Quit must wait for that.
  deferQuit: (() => boolean) | null = null;
  // File name the page is saving, so the picker result can be matched even if ArkWeb rewrites it.
  expectedSave: string = '';

  expectSave(name: string): void {
    if (name.length > 0) {
      this.expectedSave = name;
    }
  }

  // The page calls this after it can receive an opened file.
  onPullOpens: (() => void) | null = null;

  pullOpens(): void {
    if (this.onPullOpens !== null) {
      this.onPullOpens();
    }
  }

  // Theme chrome from the page (`#RRGGBB`). The status bar and navigation indicator follow it.
  setChrome(color: string): void {
    let argb: string = normalizeChrome(color);
    if (argb.length === 0 || AppStorage.get<string>('photocraftChrome') === argb) {
      return;
    }
    AppStorage.setOrCreate('photocraftChrome', argb);
    let ctx = this.context;
    if (ctx !== null) {
      applySystemBars(ctx, argb);
    }
  }

  quit(): void {
    if (this.deferQuit !== null && this.deferQuit()) {
      return;
    }
    this.terminate();
  }

  terminate(): void {
    let ctx = this.context;
    if (ctx === null) {
      return;
    }
    allowConfirmedClose();
    ctx.terminateSelf().catch((error: BusinessError) => {
      restoreCloseGuard();
      console.error(`Quit failed: ${error.code} ${error.message}`);
    });
  }
}

// Packaged resources use one private HTTPS origin so ES modules and WASM share an origin.
const SITE_ORIGIN: string = 'https://photocraft.invalid';
const START_URL: string = SITE_ORIGIN + '/index.html?webgl&cpu';
// hdc rport maps the device loopback port to Trunk on the development computer.
const DEV_ORIGIN: string = 'http://127.0.0.1:8765';
const DEV_URL: string = DEV_ORIGIN + '/index.html?webgl&cpu';
const SCRIPT_NAME: string = 'photocraft-web-a339036ca6e7497f.js';
const WASM_NAME: string = 'photocraft-web-a339036ca6e7497f_bg.wasm';

@Entry
@Component
struct Index {
  @StorageProp('photocraftDev') private devMode: boolean = false;
  @StorageProp('photocraftGpu') private gpuMode: boolean = false;
  @StorageLink('photocraftChrome') @Watch('onChromeChanged') private chrome: string = '#FF262626';
  private controller: webview.WebviewController = new webview.WebviewController();
  private downloadDelegate: webview.WebDownloadDelegate = new webview.WebDownloadDelegate();
  private context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
  private downloadPaths: Map<string, string> = new Map<string, string>();
  private downloadNames: Map<string, string> = new Map<string, string>();
  // Name the page set on the download link, so the save-picker result can be matched to that save.
  private downloadSuggested: Map<string, string> = new Map<string, string>();
  private pendingExportPath: string = '';
  private pendingExportName: string = '';
  private pendingSuggested: string = '';
  private savingExport: boolean = false;
  // File › Quit asked to exit while a save picker was still open.
  private quitAfterExport: boolean = false;
  private openPort: webview.WebMessagePort | null = null;
  private openPageReady: boolean = false;
  private host: PhotocraftHost = new PhotocraftHost();
  private fontCache: Map<string, ArrayBuffer> = new Map<string, ArrayBuffer>();
  private fontMiss: Map<string, boolean> = new Map<string, boolean>();
  private stylus: StylusAdapter = new HarmonyStylusAdapter((packet: StylusPacket): void => {
    this.publishStylusInput(packet);
  });

  private requestPageClose(): void {
    try {
      this.controller.runJavaScript('window.dispatchEvent(new Event("photocraft-request-close"))').catch((error: BusinessError) => {
        console.error(`Close request failed: ${error.code} ${error.message}`);
      });
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Close request failed: ${failure.code} ${failure.message}`);
    }
  }

  private response(body: string | Resource, mimeType: string, code: number): WebResourceResponse {
    let result = new WebResourceResponse();
    result.setResponseData(body);
    result.setResponseMimeType(mimeType);
    if (mimeType.startsWith('text/')) {
      result.setResponseEncoding('utf-8');
    }
    result.setResponseCode(code);
    result.setReasonMessage(code === 200 ? 'OK' : code === 403 ? 'Forbidden' : 'Not Found');
    result.setResponseHeader([{ headerKey: 'X-Content-Type-Options', headerValue: 'nosniff' }]);
    result.setResponseIsReady(true);
    return result;
  }

  private intercept(url: string): WebResourceResponse | null {
    let hosted = this.hostedFont(url);
    if (hosted !== null) {
      return hosted;
    }
    if (this.devMode && (url === DEV_ORIGIN || url.startsWith(DEV_ORIGIN + '/'))) {
      return null;
    }
    // Query strings are intentionally ignored; ?webgl&cpu is selected by the wrapper.
    let pathUrl: string = url.split('?')[0].split('#')[0];
    if (pathUrl === SITE_ORIGIN + '/' || pathUrl === SITE_ORIGIN + '/index.html') {
      return this.response($rawfile('index.html'), 'text/html', 200);
    }
    if (pathUrl === SITE_ORIGIN + '/' + SCRIPT_NAME) {
      return this.response($rawfile(SCRIPT_NAME), 'text/javascript', 200);
    }
    if (pathUrl === SITE_ORIGIN + '/' + WASM_NAME) {
      return this.response($rawfile(WASM_NAME), 'application/wasm', 200);
    }
    if (url.startsWith(SITE_ORIGIN + '/')) {
      return this.response('Not found', 'text/plain', 404);
    }
    // This offline build does not fetch pages or assets from the network.
    if (url.startsWith('https://') || url.startsWith('http://')) {
      return this.response('Blocked', 'text/plain', 403);
    }
    // Let ArkWeb handle blob: downloads and its own internal URLs.
    return null;
  }

  private isExternalWebUrl(url: string): boolean {
    let isWebUrl: boolean = url.startsWith('https://') || url.startsWith('http://');
    let isLocalSite: boolean = url === SITE_ORIGIN || url.startsWith(SITE_ORIGIN + '/');
    let isDevSite: boolean = this.devMode && (url === DEV_ORIGIN || url.startsWith(DEV_ORIGIN + '/'));
    return isWebUrl && !isLocalSite && !isDevSite;
  }

  private openExternalUrl(url: string): void {
    this.context.startAbility({
      action: 'ohos.want.action.viewData',
      entities: ['entity.system.browsable'],
      uri: url
    }).catch((error: BusinessError) => {
      console.error(`Browser launch failed: ${error.code} ${error.message}`);
    });
  }

  private selectDocument(result: FileSelectorResult): void {
    let documentPicker = new picker.DocumentViewPicker(this.context);
    documentPicker.select().then((uris: Array<string>) => {
      result.handleFileList(uris);
    }).catch((error: BusinessError) => {
      console.error(`File selection failed: ${error.code} ${error.message}`);
      result.handleFileList([]);
    });
  }

  aboutToAppear(): void {
    webview.WebviewController.setWebDebuggingAccess(this.devMode || this.gpuMode);
    setCloseRequest(() => this.requestPageClose());
    this.host.context = this.context;
    this.applyChrome();
    this.host.deferQuit = (): boolean => {
      if (!this.exportBusy()) {
        return false;
      }
      this.quitAfterExport = true;
      return true;
    };
    this.host.onPullOpens = (): void => {
      this.installOpenPort();
    };
    setOpenListener((): void => {
      this.deliverOpens();
    });
    this.stylus.start();
  }

  aboutToDisappear(): void {
    setCloseRequest(null);
    setOpenListener(null);
    this.stylus.stop();
  }

  private publishStylusInput(packet: StylusPacket): void {
    try {
      this.controller.runJavaScript(stylusPacketScript(packet)).catch((error: BusinessError) => {
        console.error(`Stylus input inject failed: ${error.code} ${error.message}`);
      });
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Stylus input inject failed: ${failure.code} ${failure.message}`);
    }
  }

  onChromeChanged(): void {
    this.applyChrome();
  }

  // Colour the avoid areas behind the status bar and the bottom indicator, and the page behind
  // the canvas, so a theme change does not leave a white strip.
  private applyChrome(): void {
    applySystemBars(this.context, this.chrome);
    this.paintPageChrome();
  }

  private paintPageChrome(): void {
    let css: string = this.chrome.length === 9 ? '#' + this.chrome.substring(3) : this.chrome;
    let script: string = '(function(){var c="' + css +
      '";var e=document.documentElement;if(e){e.style.background=c;}if(document.body){document.body.style.background=c;}})()';
    try {
      this.controller.runJavaScript(script).catch((error: BusinessError) => {
        console.error(`Chrome paint failed: ${error.code} ${error.message}`);
      });
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Chrome paint failed: ${failure.code} ${failure.message}`);
    }
  }

  onBackPress(): boolean {
    this.injectEscape();
    return true;
  }

  private injectEscape(): void {
    let script: string =
      '(function(){var o={key:"Escape",code:"Escape",keyCode:27,which:27,bubbles:true,cancelable:true};' +
      'var ev=function(n){return new KeyboardEvent(n,o)};' +
      'window.dispatchEvent(ev("keydown"));document.dispatchEvent(ev("keydown"));' +
      'var c=document.getElementById("photocraft_canvas");' +
      'if(c){c.focus();c.dispatchEvent(ev("keydown"));c.dispatchEvent(ev("keyup"));}' +
      'document.dispatchEvent(ev("keyup"));window.dispatchEvent(ev("keyup"));})()';
    try {
      this.controller.runJavaScript(script).catch((error: BusinessError) => {
        console.error(`Escape inject failed: ${error.code} ${error.message}`);
      });
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Escape inject failed: ${failure.code} ${failure.message}`);
    }
  }

  private bytesResponse(body: ArrayBuffer, mimeType: string): WebResourceResponse {
    let result = new WebResourceResponse();
    result.setResponseData(body);
    result.setResponseMimeType(mimeType);
    result.setResponseCode(200);
    result.setReasonMessage('OK');
    result.setResponseHeader([{ headerKey: 'X-Content-Type-Options', headerValue: 'nosniff' }]);
    result.setResponseIsReady(true);
    return result;
  }

  // egui draws its own glyphs, so the system font has to be read and handed to WASM.
  // HarmonyOS Sans is not bundled: the face says all rights reserved.
  private hostedFont(url: string): WebResourceResponse | null {
    let path: string = url.split('?')[0].split('#')[0];
    let family: string = '';
    let fileName: string = '';
    if (path.endsWith('/fonts/HarmonyOS_Sans_SC.ttf')) {
      family = 'HarmonyOS Sans SC';
      fileName = 'HarmonyOS_Sans_SC.ttf';
    } else if (path.endsWith('/fonts/HarmonyOS_Sans_TC.ttf')) {
      family = 'HarmonyOS Sans TC';
      fileName = 'HarmonyOS_Sans_TC.ttf';
    } else {
      return null;
    }
    let bytes: ArrayBuffer | null = this.loadHostFont(family, fileName);
    if (bytes === null) {
      return this.response('Not found', 'text/plain', 404);
    }
    return this.bytesResponse(bytes, 'font/ttf');
  }

  private loadHostFont(family: string, fileName: string): ArrayBuffer | null {
    let cached: ArrayBuffer | undefined = this.fontCache.get(family);
    if (cached !== undefined) {
      return cached;
    }
    if (this.fontMiss.get(family) === true) {
      return null;
    }
    let bytes: ArrayBuffer | null = this.readSystemFont(family, fileName);
    if (bytes === null) {
      bytes = this.readRawFont(fileName);
    }
    if (bytes === null) {
      this.fontMiss.set(family, true);
      console.error(`HarmonyOS font missing: ${family}`);
      return null;
    }
    this.fontCache.set(family, bytes);
    return bytes;
  }

  private systemFontPath(family: string, fileName: string): string {
    try {
      let info = font.getFontByName(family);
      if (info.path.length > 0) {
        return info.path;
      }
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Font lookup failed: ${failure.code} ${failure.message}`);
    }
    return '/system/fonts/' + fileName;
  }

  private readSystemFont(family: string, fileName: string): ArrayBuffer | null {
    let primary: string = this.systemFontPath(family, fileName);
    let fallback: string = '/system/fonts/' + fileName;
    let bytes: ArrayBuffer | null = this.readFontFile(primary);
    if (bytes === null && primary !== fallback) {
      bytes = this.readFontFile(fallback);
    }
    return bytes;
  }

  private readFontFile(path: string): ArrayBuffer | null {
    if (path.length === 0 || path.indexOf('\0') >= 0) {
      return null;
    }
    let maxBytes: number = 48 * 1024 * 1024;
    try {
      let stat = fileIo.statSync(path);
      let size: number = stat.size;
      if (size <= 12 || size > maxBytes) {
        return null;
      }
      let file: fileIo.File = fileIo.openSync(path, fileIo.OpenMode.READ_ONLY);
      try {
        let buffer = new ArrayBuffer(size);
        let read: number = fileIo.readSync(file.fd, buffer);
        if (read !== size) {
          return null;
        }
        return buffer;
      } finally {
        fileIo.closeSync(file);
      }
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Font read failed: ${path} ${failure.code} ${failure.message}`);
      return null;
    }
  }

  private readRawFont(fileName: string): ArrayBuffer | null {
    let maxBytes: number = 48 * 1024 * 1024;
    try {
      let bytes = this.context.resourceManager.getRawFileContentSync('fonts/' + fileName);
      if (bytes.byteLength <= 12 || bytes.byteLength > maxBytes) {
        return null;
      }
      return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Bundled font read failed: ${failure.code} ${failure.message}`);
      return null;
    }
  }

  private systemLangTag(): string {
    try {
      let tag: string = i18n.System.getSystemLocaleInstance().baseName;
      if (tag.length > 0) {
        return tag;
      }
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`System locale failed: ${failure.code} ${failure.message}`);
    }
    try {
      let language: string = i18n.System.getSystemLanguage();
      if (language.length > 0) {
        return language;
      }
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`System language failed: ${failure.code} ${failure.message}`);
    }
    return '';
  }

  private pageUrl(): string {
    // PhotoCraft Auto reads ?lang= once at startup. A saved Interface language still wins.
    let base: string = this.devMode ? DEV_URL : START_URL;
    if (this.gpuMode) {
      base = base.replace('webgl&cpu', 'webgl&gpu');
    }
    let tag: string = this.systemLangTag();
    if (tag.length === 0) {
      return base;
    }
    return base + '&lang=' + encodeURIComponent(tag);
  }

  private safeName(name: string): string {
    let clean: string = name.replace(/[\/\\:*?"<>|]/g, '_').trim();
    if (clean.length === 0 || clean === '.' || clean === '..') {
      return 'photocraft-export.png';
    }
    return clean;
  }

  private setUpDownloads(): void {
    this.downloadDelegate.onBeforeDownload((item: webview.WebDownloadItem) => {
      let guid: string = item.getGuid();
      let suggested: string = item.getSuggestedFileName();
      let name: string = this.safeName(suggested);
      let path: string = this.context.cacheDir + '/export-' + Date.now().toString() + '-' + name;
      this.downloadPaths.set(guid, path);
      this.downloadNames.set(guid, name);
      this.downloadSuggested.set(guid, suggested.length > 0 ? suggested : name);
      try {
        item.start(path);
      } catch (error) {
        let failure = error as BusinessError;
        console.error(`Export start failed: ${failure.code} ${failure.message}`);
        this.downloadPaths.delete(guid);
        this.downloadNames.delete(guid);
        this.downloadSuggested.delete(guid);
        this.finishExport(false, suggested.length > 0 ? suggested : name);
      }
    });
    this.downloadDelegate.onDownloadFinish((item: webview.WebDownloadItem) => {
      let guid: string = item.getGuid();
      let path: string | undefined = this.downloadPaths.get(guid);
      if (path !== undefined) {
        this.pendingExportPath = path;
        this.pendingExportName = this.downloadNames.get(guid) ?? 'photocraft-export.png';
        this.pendingSuggested = this.downloadSuggested.get(guid) ?? this.pendingExportName;
        this.saveExport();
      }
      this.downloadPaths.delete(guid);
      this.downloadNames.delete(guid);
      this.downloadSuggested.delete(guid);
    });
    this.downloadDelegate.onDownloadFailed((item: webview.WebDownloadItem) => {
      let guid: string = item.getGuid();
      let name: string = this.downloadSuggested.get(guid) ?? this.downloadNames.get(guid) ?? '';
      console.error(`Export failed: ${item.getLastErrorCode()}`);
      this.downloadPaths.delete(guid);
      this.downloadNames.delete(guid);
      this.downloadSuggested.delete(guid);
      this.finishExport(false, name);
    });
    try {
      this.controller.setDownloadDelegate(this.downloadDelegate);
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Download delegate failed: ${failure.code} ${failure.message}`);
    }
  }

  private discardPending(path: string): void {
    try {
      fileIo.unlinkSync(path);
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Export cleanup failed: ${failure.code} ${failure.message}`);
    }
    if (this.pendingExportPath === path) {
      this.pendingExportPath = '';
      this.pendingExportName = '';
      this.pendingSuggested = '';
    }
  }

  private exportBusy(): boolean {
    return this.downloadPaths.size > 0 || this.savingExport || this.pendingExportPath.length > 0;
  }

  private jsString(value: string): string {
    return value.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\r/g, '\\r').replace(/\n/g, '\\n');
  }

  // Tells the page whether the system save picker wrote the file, so File › Quit can wait for it.
  private notifyExport(saved: boolean, name: string): void {
    let result: string = saved ? 'saved' : 'cancelled';
    let script: string = '(function(){try{window.dispatchEvent(new CustomEvent("photocraft-export",{detail:{result:"' +
      result + '",name:"' + this.jsString(name) + '"}}));}catch(e){}})()';
    try {
      this.controller.runJavaScript(script).catch((error: BusinessError) => {
        console.error(`Export notify failed: ${error.code} ${error.message}`);
      });
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Export notify failed: ${failure.code} ${failure.message}`);
    }
  }

  private filePart(value: string): string {
    let slash: number = value.lastIndexOf('/');
    let back: number = value.lastIndexOf('\\');
    let cut: number = slash > back ? slash : back;
    return cut >= 0 ? value.substring(cut + 1) : value;
  }

  private namesLookSame(suggested: string, expected: string): boolean {
    if (suggested.length === 0 || expected.length === 0) {
      return false;
    }
    if (suggested === expected || this.safeName(suggested) === this.safeName(expected)) {
      return true;
    }
    let suggestedFile: string = this.filePart(suggested);
    let expectedFile: string = this.filePart(expected);
    return suggestedFile === expectedFile || this.safeName(suggestedFile) === this.safeName(expectedFile);
  }

  // Prefer the name the page armed, so a rewritten suggested name still completes that save.
  private eventName(suggested: string): string {
    let expected: string = this.host.expectedSave;
    if (this.namesLookSame(suggested, expected)) {
      this.host.expectedSave = '';
      return expected;
    }
    return suggested;
  }

  private finishExport(saved: boolean, name: string): void {
    this.notifyExport(saved, this.eventName(name));
    if (!saved) {
      this.quitAfterExport = false;
    }
  }

  private pumpDeferredQuit(): void {
    if (!this.quitAfterExport || this.exportBusy()) {
      return;
    }
    this.quitAfterExport = false;
    this.host.terminate();
  }

  private async saveExport(): Promise<void> {
    if (this.pendingExportPath.length === 0 || this.savingExport) {
      return;
    }
    this.savingExport = true;
    let path: string = this.pendingExportPath;
    let name: string = this.pendingExportName;
    let suggested: string = this.pendingSuggested.length > 0 ? this.pendingSuggested : name;
    let saved: boolean = false;
    let options = new picker.DocumentSaveOptions();
    options.newFileNames = [name];
    try {
      let uris: Array<string> = await new picker.DocumentViewPicker(this.context).save(options);
      if (uris.length === 0) {
        this.discardPending(path);
        return;
      }
      let source: fileIo.File = fileIo.openSync(path, fileIo.OpenMode.READ_ONLY);
      try {
        let destination: fileIo.File = fileIo.openSync(uris[0], fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC);
        try {
          fileIo.copyFileSync(source.fd, destination.fd);
        } finally {
          fileIo.closeSync(destination);
        }
      } finally {
        fileIo.closeSync(source);
      }
      this.discardPending(path);
      saved = true;
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Save export failed: ${failure.code} ${failure.message}`);
      this.discardPending(path);
    } finally {
      this.savingExport = false;
      this.finishExport(saved, suggested);
      this.pumpDeferredQuit();
    }
  }

  private messageTarget(): string {
    try {
      let url = this.controller.getUrl();
      let scheme = url.indexOf('://');
      if (scheme > 0) {
        let rest = url.substring(scheme + 3);
        let slash = rest.indexOf('/');
        let host = slash >= 0 ? rest.substring(0, slash) : rest;
        let query = host.indexOf('?');
        if (query >= 0) {
          host = host.substring(0, query);
        }
        return url.substring(0, scheme + 3) + host;
      }
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Open target failed: ${failure.code} ${failure.message}`);
    }
    return this.devMode ? DEV_ORIGIN : SITE_ORIGIN;
  }

  private installOpenPort(): boolean {
    if (this.openPort !== null) {
      try {
        this.openPort.close();
      } catch (error) {
        let failure = error as BusinessError;
        console.error(`Open port close failed: ${failure.code} ${failure.message}`);
      }
      this.openPort = null;
    }
    this.openPageReady = false;
    try {
      let ports = this.controller.createWebMessagePorts();
      ports[1].onMessageEvent((message: string | ArrayBuffer): void => {
        if (message instanceof ArrayBuffer || message !== 'ready') {
          return;
        }
        this.openPageReady = true;
        this.deliverOpens();
      });
      this.openPort = ports[1];
      this.controller.postMessage('photocraft-open-port', [ports[0]], this.messageTarget());
      return true;
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Open port failed: ${failure.code} ${failure.message}`);
      return false;
    }
  }

  private deliverOpens(): void {
    if (this.openPort === null) {
      return;
    }
    if (!this.openPageReady) {
      return;
    }
    let files = takeOpens();
    let port = this.openPort;
    for (let file of files) {
      try {
        port.postMessageEvent('open:' + file.name);
        port.postMessageEvent(file.bytes);
      } catch (error) {
        let failure = error as BusinessError;
        console.error(`Open deliver failed: ${failure.code} ${failure.message}`);
      }
    }
  }

  build() {
    Stack() {
      Web({ src: this.pageUrl(), controller: this.controller })
      .javaScriptAccess(true)
      .domStorageAccess(true)
      .fileAccess(false)
      .zoomAccess(false)
      .width('100%')
      .height('100%')
      .javaScriptProxy({
        object: this.host,
        name: 'photocraftHost',
        methodList: ['quit', 'expectSave', 'pullOpens', 'setChrome'],
        controller: this.controller
      })
      .onControllerAttached(() => {
        this.host.context = this.context;
        this.setUpDownloads();
      })
      .onPageEnd(() => {
        this.stylus.publishState();
        this.applyChrome();
      })
      .onLoadIntercept((event) => {
        if (!event) {
          return false;
        }
        let url: string = event.data.getRequestUrl();
        if (this.isExternalWebUrl(url)) {
          this.openExternalUrl(url);
          return true;
        }
        return false;
      })
      .onInterceptRequest((event) => {
        if (!event) {
          return null;
        }
        return this.intercept(event.request.getRequestUrl());
      })
      .onShowFileSelector((event) => {
        if (!event) {
          return false;
        }
        this.selectDocument(event.result);
        return true;
      })
      .backgroundColor(this.chrome);
    }
    .width('100%')
    .height('100%')
    .backgroundColor(this.chrome);
  }
}
