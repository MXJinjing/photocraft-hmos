import native from 'libphotocraft.so';
import { LOADING_CHROME } from '../SystemBars';
import { isFiniteHoverPoint, penHoverLeaveMessage, penHoverMoveMessage } from '../input/PenHoverPreview';
import { SUB_WINDOW_KEY, taggedForWindow } from '../SubWindowHost';

class BlurInput {
  kind: string = 'blur';
}

// Content of a native sub-window (floating document window). One XComponent whose id carries
// the window key: the native layer binds its surface/input to that egui viewport.
@Entry
@Component
struct SubWindow {
  @LocalStorageProp(SUB_WINDOW_KEY) private winKey: number = 0;
  @StorageLink('photocraftChrome') private chrome: string = LOADING_CHROME;

  private sendPenHoverMove(event: HoverEvent): void {
    const x = Number(event.x);
    const y = Number(event.y);
    if (!isFiniteHoverPoint(x, y)) { return; }
    const ui = this.getUIContext();
    native.input(taggedForWindow(penHoverMoveMessage(ui.vp2px(x), ui.vp2px(y)), this.winKey));
  }

  build() {
    Stack() {
      XComponent({ id: `pc-win-${this.winKey}`, type: XComponentType.SURFACE, libraryname: 'photocraft' })
        .width('100%').height('100%')
        .onLoad((context?: object): void => {
          // Bind this XComponent's callbacks (touch/pen/mouse/key/axis/surface) to its window key.
          if (context !== undefined) { native.registerWindow(context); }
        })
        .onHoverMove((event: HoverEvent): void => { this.sendPenHoverMove(event); })
        .onHover((isHover: boolean): void => {
          if (!isHover) { native.input(taggedForWindow(penHoverLeaveMessage(), this.winKey)); }
        })
        .onBlur((): void => { native.input(taggedForWindow(JSON.stringify(new BlurInput()), this.winKey)); })
        .focusable(true).defaultFocus(true).focusOnTouch(true)
    }.width('100%').height('100%').backgroundColor(this.chrome)
  }
}
