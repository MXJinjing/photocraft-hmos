import { inputMethod } from '@kit.IMEKit';
import { BusinessError } from '@kit.BasicServicesKit';

export class NativeImeState {
  active: boolean = false;
  focus: string = '';
  text: string = '';
  start: number = 0;
  end: number = 0;
  x: number = 0;
  y: number = 0;
  width: number = 1;
  height: number = 1;
  password: boolean = false;
  showRequest: number = 0;
}
class ImeInput {
  kind: string;
  focus: string;
  text: string = '';
  count: number = 0;
  start: number = 0;
  end: number = 0;
  code: number = 0;
  constructor(kind: string, focus: string) { this.kind = kind; this.focus = focus; }
}

// Bind the system IME to the focused egui editor, without introducing an ArkUI text field.
export class NativeImeAdapter {
  private controller: inputMethod.InputMethodController = inputMethod.getController();
  private state: NativeImeState = new NativeImeState();
  private attached: string = '';
  private revision: number = 0;
  private running: boolean = false;
  private listening: boolean = false;
  private preview: string = '';
  private shownRequest: number = 0;
  private originX: number = 0;
  private originY: number = 0;
  private emit: (message: string) => void;

  constructor(emit: (message: string) => void) { this.emit = emit; }
  update(state: NativeImeState): void {
    this.state = state;
    this.revision++;
    this.reconcile();
  }
  setOrigin(x: number, y: number): void {
    this.originX = x; this.originY = y;
    this.revision++;
    this.reconcile();
  }
  stop(): void { this.update(new NativeImeState()); }
  private send(kind: string, text: string = '', count: number = 0, code: number = 0,
    start: number = 0, end: number = 0): void {
    // 焦点标识必须与已绑定的编辑框一致，防止迟到的输入法事件写入另一个文本框。
    if (!this.state.active || this.attached !== this.state.focus) { return; }
    let input = new ImeInput(kind, this.attached);
    input.text = text; input.count = count; input.code = code; input.start = start; input.end = end;
    this.emit(JSON.stringify(input));
  }
  private listen(): void {
    this.listening = true;
    this.controller.on('insertText', (text: string): void => {
      this.send('ime_commit', text); this.preview = '';
    });
    this.controller.on('deleteLeft', (length: number): void => this.send('ime_delete_left', '', length));
    this.controller.on('deleteRight', (length: number): void => this.send('ime_delete_right', '', length));
    this.controller.on('sendFunctionKey', (_key: inputMethod.FunctionKey): void => this.send('ime_key'));
    this.controller.on('moveCursor', (direction: inputMethod.Direction): void => this.send('ime_key', '', 0, direction));
    this.controller.on('selectByRange', (range: inputMethod.Range): void => {
      this.send('ime_select', '', 0, 0, range.start, range.end);
    });
    this.controller.on('getLeftTextOfCursor', (length: number): string => {
      return this.state.text.substring(Math.max(0, this.state.start - length), this.state.start);
    });
    this.controller.on('getRightTextOfCursor', (length: number): string => {
      return this.state.text.substring(this.state.end, this.state.end + length);
    });
    this.controller.on('getTextIndexAtCursor', (): number => this.state.end);
    this.controller.on('setPreviewText', (text: string, range: inputMethod.Range): void => {
      if (this.preview.length === 0 && range.start >= 0 && range.end >= 0) {
        this.send('ime_select', '', 0, 0, range.start, range.end);
      }
      this.preview = text; this.send('ime_preedit', text);
    });
    this.controller.on('finishTextPreview', (): void => {
      if (this.preview.length > 0) { this.send('ime_commit', this.preview); this.preview = ''; }
    });
  }
  private unlisten(): void {
    if (!this.listening) { return; }
    this.listening = false;
    this.controller.off('insertText'); this.controller.off('deleteLeft'); this.controller.off('deleteRight');
    this.controller.off('sendFunctionKey'); this.controller.off('moveCursor'); this.controller.off('selectByRange');
    this.controller.off('getLeftTextOfCursor'); this.controller.off('getRightTextOfCursor');
    this.controller.off('getTextIndexAtCursor'); this.controller.off('setPreviewText');
    this.controller.off('finishTextPreview');
  }
  private cursor(): inputMethod.CursorInfo {
    return { left: this.originX + this.state.x, top: this.originY + this.state.y,
      width: this.state.width, height: this.state.height };
  }
  private async reconcile(): Promise<void> {
    if (this.running) { return; }
    this.running = true;
    try {
      let revision: number;
      do {
        revision = this.revision;
        let focus: string = this.state.active ? this.state.focus : '';
        if (this.attached.length > 0 && this.attached !== focus) {
          this.unlisten(); this.preview = '';
          await this.controller.detach(); this.attached = ''; this.shownRequest = 0;
        }
        if (focus.length > 0 && this.attached.length === 0) {
          let requested: number = this.state.showRequest;
          await this.controller.attach(requested > 0, {
            inputAttribute: { textInputType: this.state.password ? inputMethod.TextInputType.VISIBLE_PASSWORD : inputMethod.TextInputType.TEXT,
              enterKeyType: inputMethod.EnterKeyType.NEWLINE },
            cursorInfo: this.cursor(), selection: { start: this.state.start, end: this.state.end }
          });
          this.attached = focus;
          this.shownRequest = requested;
          this.listen();
        }
        if (this.state.active && this.attached === this.state.focus) {
          if (this.state.showRequest > this.shownRequest) {
            this.shownRequest = this.state.showRequest;
            await this.controller.showTextInput();
          }
          await this.controller.changeSelection(this.state.text, this.state.start, this.state.end);
          await this.controller.updateCursor(this.cursor());
        }
      } while (revision !== this.revision);
    } catch (error) {
      let failure = error as BusinessError;
      console.error(`Native IME: ${failure.code} ${failure.message}`);
    } finally { this.running = false; }
  }
}
