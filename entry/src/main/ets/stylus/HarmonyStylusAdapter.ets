// HarmonyOS implements the shared contract; SDK names never enter the canvas action layer.
import { BusinessError } from '@kit.BasicServicesKit';
import { inputDevice } from '@kit.InputKit';
import { stylusInteraction } from '@kit.Penkit';
import { StylusAction, StylusAdapter, StylusPacket } from './StylusProtocol';

export class HarmonyStylusAdapter implements StylusAdapter {
  private emit: (packet: StylusPacket) => void;
  private active: boolean = false;
  private generation: number = 0;
  private connected: boolean = false;
  private connectedKnown: boolean = false;
  private poll: number = -1;
  private doubleTapListening: boolean = false;
  private squeezeListening: boolean = false;
  private deviceListening: boolean = false;

  constructor(emit: (packet: StylusPacket) => void) {
    this.emit = emit;
  }

  private readonly onDoubleTap = (_event: stylusInteraction.DoubleTapEvent): void => {
    if (this.active) {
      this.emit(StylusPacket.action(StylusAction.DoubleTap, Date.now()));
    }
  };
  private readonly onSqueeze = (_event: stylusInteraction.SqueezeEvent): void => {
    if (this.active) {
      this.emit(StylusPacket.action(StylusAction.LongPress, Date.now()));
    }
  };
  private readonly onDeviceChange = (_event: inputDevice.DeviceListener): void => {
    this.refreshConnection();
  };

  start(): void {
    if (this.active) {
      return;
    }
    this.active = true;
    this.generation++;
    this.connectedKnown = false;
    try {
      stylusInteraction.on('doubleTap', this.onDoubleTap);
      this.doubleTapListening = true;
    } catch (error) {
      this.reportError('doubleTap listen', error as BusinessError);
    }
    try {
      stylusInteraction.on('squeeze', this.onSqueeze);
      this.squeezeListening = true;
    } catch (error) {
      this.reportError('squeeze listen', error as BusinessError);
    }
    try {
      inputDevice.on('change', this.onDeviceChange);
      this.deviceListening = true;
    } catch (error) {
      this.reportError('device listen', error as BusinessError);
    }
    this.refreshConnection();
    this.poll = setInterval((): void => this.refreshConnection(), 1000);
  }

  stop(): void {
    if (!this.active) {
      return;
    }
    this.active = false;
    this.generation++;
    if (this.poll >= 0) {
      clearInterval(this.poll);
      this.poll = -1;
    }
    if (this.doubleTapListening) {
      try {
        stylusInteraction.off('doubleTap', this.onDoubleTap);
      } catch (error) {
        this.reportError('doubleTap unlisten', error as BusinessError);
      }
      this.doubleTapListening = false;
    }
    if (this.squeezeListening) {
      try {
        stylusInteraction.off('squeeze', this.onSqueeze);
      } catch (error) {
        this.reportError('squeeze unlisten', error as BusinessError);
      }
      this.squeezeListening = false;
    }
    if (this.deviceListening) {
      try {
        inputDevice.off('change', this.onDeviceChange);
      } catch (error) {
        this.reportError('device unlisten', error as BusinessError);
      }
      this.deviceListening = false;
    }
  }

  publishState(): void {
    this.connectedKnown = false;
    this.refreshConnection();
  }

  private isStylusDevice(id: number): boolean {
    try {
      if (inputDevice.getKeyboardTypeSync(id) === inputDevice.KeyboardType.HANDWRITING_PEN) {
        return true;
      }
    } catch (error) {
      // Some pointer devices have no keyboard type; use their device name below.
    }
    try {
      let name: string = inputDevice.getDeviceInfoSync(id).name.toLowerCase();
      return name.includes('pencil') || name.includes('stylus') || name.includes('handwriting') || name.includes('wacom');
    } catch (error) {
      return false;
    }
  }

  private refreshConnection(): void {
    if (!this.active) {
      return;
    }
    let generation: number = this.generation;
    inputDevice.getDeviceList().then((ids: Array<number>) => {
      if (!this.active || generation !== this.generation) {
        return;
      }
      let connected: boolean = false;
      for (let i = 0; i < ids.length; i++) {
        if (this.isStylusDevice(ids[i])) {
          connected = true;
          break;
        }
      }
      if (!this.connectedKnown || connected !== this.connected) {
        this.connectedKnown = true;
        this.connected = connected;
        this.emit(StylusPacket.connection(connected));
      }
    }).catch((error: BusinessError) => this.reportError('device list', error));
  }

  private reportError(operation: string, error: BusinessError): void {
    console.error(`HarmonyOS stylus ${operation}: ${error.code} ${error.message}`);
  }
}
