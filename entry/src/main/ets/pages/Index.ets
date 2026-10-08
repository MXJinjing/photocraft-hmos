import { common } from '@kit.AbilityKit';
import { fileIo, picker } from '@kit.CoreFileKit';
import { webview } from '@kit.ArkWeb';
import { BusinessError } from '@kit.BasicServicesKit';

// The page uses one private HTTPS origin so ES modules and WASM have the same origin.
const SITE_ORIGIN: string = 'https://photocraft.invalid';
const START_URL: string = SITE_ORIGIN + '/index.html?webgl&cpu';
const SCRIPT_NAME: string = 'photocraft-web-a705b2188e2fccf8.js';
const WASM_NAME: string = 'photocraft-web-a705b2188e2fccf8_bg.wasm';

@Entry
@Component
struct Index {
  private controller: webview.WebviewController = new webview.WebviewController();
  private downloadDelegate: webview.WebDownloadDelegate = new webview.WebDownloadDelegate();
  private context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
  private downloadPaths: Map<string, string> = new Map<string, string>();
  private downloadNames: Map<string, string> = new Map<string, string>();
  @State private pendingExportPath: string = '';
  @State private pendingExportName: string = '';
  @State private exportStatus: string = '';
  @State private savingExport: boolean = false;

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
    return isWebUrl && !isLocalSite;
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
      let name: string = this.safeName(item.getSuggestedFileName());
      let path: string = this.context.cacheDir + '/export-' + Date.now().toString() + '-' + name;
      this.downloadPaths.set(guid, path);
      this.downloadNames.set(guid, name);
      try {
        item.start(path);
      } catch (error) {
        let failure = error as BusinessError;
        console.error(`Export start failed: ${failure.code} ${failure.message}`);
        this.downloadPaths.delete(guid);
        this.downloadNames.delete(guid);
        this.exportStatus = '导出失败，请重试';
      }
    });
    this.downloadDelegate.onDownloadFinish((item: webview.WebDownloadItem) => {
      let guid: string = item.getGuid();
      let path: string | undefined = this.downloadPaths.get(guid);
      if (path !== undefined) {
        this.pendingExportPath = path;
        this.pendingExportName = this.downloadNames.get(guid) ?? 'photocraft-export.png';
        this.exportStatus = '正在选择保存位置…';
        this.saveExport();
      }
      this.downloadPaths.delete(guid);
      this.downloadNames.delete(guid);
    });
    this.downloadDelegate.onDownloadFailed((item: webview.WebDownloadItem) => {
      let guid: string = item.getGuid();
      console.error(`Export failed: ${item.getLastErrorCode()}`);
      this.downloadPaths.delete(guid);
      this.downloadNames.delete(guid);
      this.exportStatus = '导出失败，请重试';
    });
    try {
      this.controller.setDownloadDelegate(this.downloadDelegate);
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Download delegate failed: ${failure.code} ${failure.message}`);
      this.exportStatus = '无法启用导出，请重启应用';
    }
  }

  private async saveExport(): Promise<void> {
    if (this.pendingExportPath.length === 0 || this.savingExport) {
      return;
    }
    this.savingExport = true;
    this.exportStatus = '正在选择保存位置…';
    let path: string = this.pendingExportPath;
    let name: string = this.pendingExportName;
    let options = new picker.DocumentSaveOptions();
    options.newFileNames = [name];
    try {
      let uris: Array<string> = await new picker.DocumentViewPicker(this.context).save(options);
      if (uris.length === 0) {
        this.exportStatus = '保存已取消，点击重试';
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
      fileIo.unlinkSync(path);
      if (this.pendingExportPath === path) {
        this.pendingExportPath = '';
        this.pendingExportName = '';
        this.exportStatus = '已保存';
      }
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Save export failed: ${failure.code} ${failure.message}`);
      this.exportStatus = '保存未完成，点击重试';
    } finally {
      this.savingExport = false;
    }
  }

  build() {
    Stack({ alignContent: Alignment.TopEnd }) {
      Web({ src: START_URL, controller: this.controller })
        .javaScriptAccess(true)
        .domStorageAccess(true)
        .fileAccess(false)
        .width('100%')
        .height('100%')
        .onControllerAttached(() => {
          this.setUpDownloads();
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
        });
      if (this.pendingExportPath.length > 0 && !this.savingExport) {
        Button(this.exportStatus)
          .onClick(() => this.saveExport())
          .enabled(!this.savingExport)
          .margin(12)
          .backgroundColor('#315A93');
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#262626');
  }
}
