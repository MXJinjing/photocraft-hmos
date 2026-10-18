// HarmonyOS implements the shared contract; SDK names never enter the canvas action layer.
import { BusinessError } from '@kit.BasicServicesKit';
import { inputDevice } from '@kit.InputKit';
import { ble, connection, hid } from '@kit.ConnectivityKit';
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
  private deviceRevision: number = 0;
  private infoPending: boolean = false;
  private accessRequested: boolean = false;
  private accessGranted: boolean = false;
  private requestInfoAccess: (() => Promise<boolean>) | undefined;
  private poll: number = -1;
  private startupDelay: number = -1;
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
    this.infoPending = false;
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
    // Defer the first InputKit scan so surface/GPU startup is not racing Binder calls
    // on the ArkTS thread (PC 2in1 emulator has shown APP_INPUT_BLOCK here).
    let generation: number = this.generation;
    this.startupDelay = setTimeout((): void => {
      this.startupDelay = -1;
      if (this.active && generation === this.generation) {
        this.refreshConnection();
      }
    }, 1500);
    this.poll = setInterval((): void => this.refreshConnection(), 5000);
  }

  stop(): void {
    if (!this.active) {
      return;
    }
    this.active = false;
    this.generation++;
    if (this.startupDelay >= 0) {
      clearTimeout(this.startupDelay);
      this.startupDelay = -1;
    }
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

  // Async only: Sync InputKit probes can block the ArkTS thread on some emulators
  // (APP_INPUT_BLOCK). Prefer Promise APIs so the UI can keep marking input.
  private async isStylusDevice(id: number): Promise<boolean> {
    try {
      let keyboardType: inputDevice.KeyboardType = await inputDevice.getKeyboardType(id);
      if (keyboardType === inputDevice.KeyboardType.HANDWRITING_PEN) {
        return true;
      }
    } catch (error) {
      // Some pointer devices have no keyboard type; use their device name below.
    }
    try {
      let info: inputDevice.InputDeviceData = await inputDevice.getDeviceInfo(id);
      let name: string = info.name.toLowerCase();
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
    this.scanDevices(generation, scan).catch((error: BusinessError) => this.reportError('device scan', error));
  }

  private async scanDevices(generation: number, scan: number): Promise<void> {
    let ids: Array<number>;
    try {
      ids = await inputDevice.getDeviceList();
    } catch (error) {
      this.reportError('device list', error as BusinessError);
      return;
    }
    if (!this.active || generation !== this.generation || scan !== this.scan) {
      return;
    }
    let id: number = -1;
    let penCount: number = 0;
    for (let i = 0; i < ids.length; i++) {
      if (await this.isStylusDevice(ids[i])) {
        if (!this.active || generation !== this.generation || scan !== this.scan) {
          return;
        }
        if (id < 0) { id = ids[i]; }
        penCount++;
      }
    }
    let info: inputDevice.InputDeviceData | undefined = undefined;
    if (id >= 0) {
      try { info = await inputDevice.getDeviceInfo(id); } catch (error) { /* Name may be unavailable. */ }
    }
    if (!this.active || generation !== this.generation || scan !== this.scan) {
      return;
    }
    let name: string | undefined = info?.name.trim() || undefined;
    if (id !== this.deviceId) {
      this.deviceId = id;
      this.deviceRevision++;
      this.publish(id >= 0, name, undefined);
    } else if (!this.accessGranted) {
      this.publish(id >= 0, name, undefined);
    } else {
      this.publish(id >= 0, this.deviceName, this.batteryPercent);
    }
    if (id < 0) { return; }
    // Do not auto-prompt for Bluetooth during startup: on PC 2in1 emulator the
    // permission sheet can fail to appear while still holding input, which trips
    // APP_INPUT_BLOCK. Battery enrichment runs only after an earlier grant.
    if (!this.accessRequested && this.requestInfoAccess !== undefined) {
      this.accessRequested = true;
      console.info('HarmonyOS stylus: deferring Bluetooth permission until pen settings need battery');
    } else if (this.accessGranted && info !== undefined && !this.infoPending) {
      await this.refreshDeviceInfo(info, generation, this.deviceRevision, penCount);
    }
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

  private async refreshDeviceInfo(info: inputDevice.InputDeviceData, generation: number, revision: number, penCount: number): Promise<void> {
    this.infoPending = true;
    let name: string | undefined = info.name.trim() || undefined;
    let battery: number | undefined = undefined;
    let ids: Array<string> = [];
    // HID and BLE are independent: a missing HID profile must not disable BLE pens.
    try { ids = hid.createHidHostProfile().getConnectedDevices(); } catch (error) { /* HID unsupported or off. */ }
    try {
      for (let id of ble.getConnectedBLEDevices()) {
        if (!ids.some((existing: string): boolean => existing.toLowerCase() === id.toLowerCase())) { ids.push(id); }
      }
    } catch (error) { /* BLE unsupported or off. */ }
    let addressMatches: Array<string> = [];
    let nameMatches: Array<string> = [];
    let penMatches: Array<string> = [];
    for (let id of ids) {
      if (id.toLowerCase() === info.uniq?.trim().toLowerCase() || id.toLowerCase() === info.phys?.trim().toLowerCase()) {
        addressMatches.push(id);
      }
      try {
        let remoteName = connection.getRemoteDeviceName(id).trim().toLowerCase();
        if (name !== undefined && remoteName === name.toLowerCase()) { nameMatches.push(id); }
        if (remoteName.includes('pencil') || remoteName.includes('stylus') || remoteName.includes('handwriting') ||
          remoteName.includes('wacom') || remoteName.includes('手写笔')) { penMatches.push(id); }
      } catch (error) { /* One device without a name must not hide other pens. */ }
    }
    // Prefer device identity. InputKit often supplies a driver name rather than the Bluetooth name.
    // The fallback requires exactly one input pen and one connected, recognisable remote pen.
    let matches = addressMatches.length > 0 ? addressMatches : nameMatches.length > 0 ? nameMatches :
      penCount === 1 ? penMatches : [];
    if (matches.length === 1) {
      let id = matches[0];
      try { name = connection.getRemoteDeviceName(id).trim() || name; } catch (error) { /* Keep InputKit name. */ }
      try {
        let value = (await connection.getRemoteDeviceBatteryInfo(id)).batteryLevel;
        if (Number.isInteger(value) && value >= 0 && value <= 100) { battery = value; }
      } catch (error) { this.reportError('battery read', error as BusinessError); }
    }
    if (this.active && generation === this.generation) {
      this.infoPending = false;
      // A slower battery read must not be discarded by a later poll. Only device/lifetime changes invalidate it.
      if (revision === this.deviceRevision) { this.publish(true, name, battery); }
    }
  }

  private reportError(operation: string, error: BusinessError): void {
    console.error(`HarmonyOS stylus ${operation}: ${error.code} ${error.message}`);
  }
}
