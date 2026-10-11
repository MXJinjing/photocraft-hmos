// Encode stylus hover as pointer moves without a press. ArkUI onHoverMove supplies
// component-local vp; callers convert to physical pixels before encoding.
export class PenHoverSample {
  pressure: number = 0;
  tiltX: number = 0;
  tiltY: number = 0;
  eraser: boolean = false;
}

export class PenHoverMove {
  kind: string = 'pointer';
  source: string = 'pen';
  action: number = 2;
  x: number = 0;
  y: number = 0;
  sample: PenHoverSample = new PenHoverSample();
}

export class PenHoverLeave {
  kind: string = 'blur';
  x: number = 0;
  y: number = 0;
}

export function penHoverMoveMessage(xPx: number, yPx: number): string {
  let message = new PenHoverMove();
  message.x = xPx;
  message.y = yPx;
  return JSON.stringify(message);
}

export function penHoverLeaveMessage(xPx: number = 0, yPx: number = 0): string {
  let message = new PenHoverLeave();
  message.x = xPx;
  message.y = yPx;
  return JSON.stringify(message);
}

export function isFiniteHoverPoint(x: number, y: number): boolean {
  return Number.isFinite(x) && Number.isFinite(y);
}
