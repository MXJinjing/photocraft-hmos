export class DropPoint {
  x: number;
  y: number;
  constructor(x: number, y: number) { this.x = x; this.y = y; }
}

// DragEvent coordinates are window vp; XComponent and the overlay share a local origin.
export function localDropPoint(x: number, y: number, originX: number, originY: number): DropPoint | null {
  const localX = x - originX;
  const localY = y - originY;
  if (!Number.isFinite(localX) || !Number.isFinite(localY)) { return null; }
  return new DropPoint(localX, localY);
}

export class DropTargetFeedback {
  sequence: number = 0;
  mode: string = '';
  left: number = 0;
  top: number = 0;
  width: number = 0;
  height: number = 0;
}

export class DropPreview {
  visible: boolean = false;
  sequence: number = 0;
  mode: string = '';
  left: number = 0;
  top: number = 0;
  width: number = 0;
  height: number = 0;

  move(): number {
    if (!this.visible) { this.mode = ''; }
    this.visible = true;
    this.sequence = this.sequence >= 2147483647 ? 1 : this.sequence + 1;
    return this.sequence;
  }

  hide(): void {
    this.visible = false;
    this.mode = '';
  }

  apply(target: DropTargetFeedback, pixelsPerVp: number): void {
    if (!this.visible || target.sequence !== this.sequence ||
      (target.mode !== 'place' && target.mode !== 'open') || !Number.isFinite(pixelsPerVp) || pixelsPerVp <= 0) { return; }
    if (target.mode === 'place' && (!Number.isFinite(target.left) || !Number.isFinite(target.top) ||
      !Number.isFinite(target.width) || !Number.isFinite(target.height) || target.width <= 0 || target.height <= 0)) { return; }
    this.mode = target.mode;
    this.left = target.left / pixelsPerVp;
    this.top = target.top / pixelsPerVp;
    this.width = target.width / pixelsPerVp;
    this.height = target.height / pixelsPerVp;
  }
}
