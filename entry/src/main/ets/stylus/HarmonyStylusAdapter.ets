// HarmonyOS implements the shared contract; SDK names never enter the canvas action layer.
import { BusinessError } from '@kit.BasicServicesKit';
import { inputDevice } from '@kit.InputKit';
import { connection, hid } from '@kit.ConnectivityKit';
import { stylusInteraction } from '@kit.Penkit';
import { StylusAction, StylusAdapter, StylusPacket } from './StylusProtocol';

export class HarmonyStylusAdapter implements StylusAdapter {
  private emit: (packet: StylusPacket) => void;
  private active: boolean = false;
  private generation: number = 0;
  private connected: boolean = false;
  private connectedKnown: boolean = false;
  private deviceId: number = -1;
  private deviceName: string | undefined = undefined;
  private batteryPercent: number | undefined = undefined;
  private scan: number = 0;
  private accessRequested: boolean = false;
  private accessGranted: boolean = false;
  private requestInfoAccess: (() => Promise<boolean>) | undefined;
  private poll: number = -1;
  private doubleTapListening: boolean = false;
  private squeezeListening: boolean = false;
  private deviceListening: boolean = false;

  constructor(emit: (packet: StylusPacket) => void, requestInfoAccess?: () => Promise<boolean>) {
    this.emit = emit;
    this.requestInfoAccess = requestInfoAccess;
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
    this.accessRequested = false;
    this.accessGranted = false;
    this.deviceId = -1;
    this.deviceName = undefined;
    this.batteryPercent = undefined;
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
    let scan: number = ++this.scan;
    inputDevice.getDeviceList().then((ids: Array<number>) => {
      if (!this.active || generation !== this.generation || scan !== this.scan) {
        return;
      }
      let id: number = -1;
      for (let i = 0; i < ids.length; i++) {
        if (this.isStylusDevice(ids[i])) {
          id = ids[i];
          break;
        }
      }
      let info: inputDevice.InputDeviceData | undefined = undefined;
      if (id >= 0) {
        try { info = inputDevice.getDeviceInfoSync(id); } catch (error) { /* Name may be unavailable. */ }
      }
      let name: string | undefined = info?.name.trim() || undefined;
      if (id !== this.deviceId) {
        this.deviceId = id;
        this.publish(id >= 0, name, undefined);
      } else if (!this.accessGranted) {
        this.publish(id >= 0, name, undefined);
      } else {
        this.publish(id >= 0, this.deviceName, this.batteryPercent);
      }
      if (id < 0) { return; }
      if (!this.accessRequested && this.requestInfoAccess !== undefined) {
        this.accessRequested = true;
        this.requestInfoAccess().then((granted: boolean) => {
          if (!this.active || generation !== this.generation) { return; }
          this.accessGranted = granted;
          if (granted) { this.refreshConnection(); }
        }).catch((error: BusinessError) => this.reportError('Bluetooth permission', error));
      } else if (this.accessGranted && info !== undefined) {
        this.refreshDeviceInfo(info, generation, scan);
      }
    }).catch((error: BusinessError) => this.reportError('device list', error));
  }

  private publish(connected: boolean, name?: string, battery?: number): void {
    if (!this.connectedKnown || connected !== this.connected || name !== this.deviceName || battery !== this.batteryPercent) {
      this.connectedKnown = true;
      this.connected = connected;
      this.deviceName = connected ? name : undefined;
      this.batteryPercent = connected ? battery : undefined;
      this.emit(StylusPacket.connection(connected, this.deviceName, this.batteryPercent));
    }
  }

  private async refreshDeviceInfo(info: inputDevice.InputDeviceData, generation: number, scan: number): Promise<void> {
    let name: string | undefined = info.name.trim() || undefined;
    let battery: number | undefined = undefined;
    try {
      // Only connected HID devices are candidates: paired, disconnected pens must not supply stale data.
      let ids = hid.createHidHostProfile().getConnectedDevices();
      let matches: Array<string> = [];
      for (let id of ids) {
        let remoteName = connection.getRemoteDeviceName(id);
        if (id.toLowerCase() === info.uniq?.toLowerCase() || id.toLowerCase() === info.phys?.toLowerCase() ||
          (name !== undefined && remoteName.toLowerCase() === name.toLowerCase())) {
          matches.push(id);
        }
      }
      // Ambiguous matches never guess which pen owns the battery.
      if (matches.length === 1) {
        let id = matches[0];
        name = connection.getRemoteDeviceName(id).trim() || name;
        let value = (await connection.getRemoteDeviceBatteryInfo(id)).batteryLevel;
        if (Number.isInteger(value) && value >= 0 && value <= 100) { battery = value; }
      }
    } catch (error) {
      // Bluetooth off, denied or unsupported: keep InputKit's name and show unavailable battery.
    }
    if (this.active && generation === this.generation && scan === this.scan) {
      this.publish(true, name, battery);
    }
  }

  private reportError(operation: string, error: BusinessError): void {
    console.error(`HarmonyOS stylus ${operation}: ${error.code} ${error.message}`);
  }
}
