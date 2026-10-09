// Platform-independent host contract. Keep in sync with docs/stylus-input.md.
export const STYLUS_HOST_EVENT: string = 'photocraft-stylus-input';
export const STYLUS_PROTOCOL_VERSION: number = 1;
export const STYLUS_STATE_PROPERTY: string = '__photocraftStylusState';

export enum StylusEventType {
  Pointer = 'pointer',
  BarrelButton = 'barrelButton',
  Gesture = 'gesture',
  Connection = 'connection'
}

export enum StylusAction {
  DoubleTap = 'doubleTap',
  LongPress = 'longPress'
}

export enum StylusPointerSource {
  Mouse = 'mouse',
  Pen = 'pen',
  Touch = 'touch'
}

export class StylusSample {
  pressure: number = 1;
  tiltX: number = 0;
  tiltY: number = 0;
  rotation: number = 0;
  eraser: boolean = false;
}

export class StylusPacket {
  version: number = STYLUS_PROTOCOL_VERSION;
  type: StylusEventType;
  gesture?: StylusAction;
  timestampMs?: number;
  connected?: boolean;
  source?: StylusPointerSource;
  sample?: StylusSample | null;
  pressed?: boolean;

  constructor(type: StylusEventType) {
    this.type = type;
  }

  static action(action: StylusAction, timestampMs: number): StylusPacket {
    let packet = new StylusPacket(StylusEventType.Gesture);
    packet.gesture = action;
    packet.timestampMs = timestampMs;
    return packet;
  }

  static connection(connected: boolean): StylusPacket {
    let packet = new StylusPacket(StylusEventType.Connection);
    packet.connected = connected;
    return packet;
  }

  static pointer(source: StylusPointerSource, sample: StylusSample | null): StylusPacket {
    let packet = new StylusPacket(StylusEventType.Pointer);
    packet.source = source;
    packet.sample = sample;
    return packet;
  }

  static barrelButton(pressed: boolean, timestampMs: number): StylusPacket {
    let packet = new StylusPacket(StylusEventType.BarrelButton);
    packet.pressed = pressed;
    packet.timestampMs = timestampMs;
    return packet;
  }
}

export interface StylusAdapter {
  start(): void;
  stop(): void;
  // Publish current connection state again after the web consumer reloads.
  publishState(): void;
}

export function stylusPacketScript(packet: StylusPacket): string {
  // Connection can arrive before WASM installs its listener. Cache just the latest state for
  // replay by the consumer; gestures are never replayed after startup.
  return '(function(){let packet=' + JSON.stringify(packet) +
    ';if(packet.type==="connection"){window.' + STYLUS_STATE_PROPERTY + '=packet;}' +
    'window.dispatchEvent(new CustomEvent("' + STYLUS_HOST_EVENT + '",{detail:packet}));})()';
}
