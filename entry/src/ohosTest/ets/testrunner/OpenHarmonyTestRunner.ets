import { abilityDelegatorRegistry, TestRunner, Driver, PointerMatrix } from '@kit.TestKit';
import native from 'libphotocraft.so';

export default class OpenHarmonyTestRunner implements TestRunner {
  onPrepare(): void {}

  private async wait(ms: number): Promise<void> {
    await new Promise<void>((resolve) => { setTimeout(resolve, ms); });
  }

  async onRun(): Promise<void> {
    const delegator = abilityDelegatorRegistry.getAbilityDelegator();
    try {
      await delegator.startAbility({
        bundleName: abilityDelegatorRegistry.getArguments().bundleName,
        moduleName: 'entry', abilityName: 'EntryAbility'
      });
      await this.wait(8000);
      native.command('{"id":"file.new","params":{"width":512,"height":512}}');
      native.command('{"id":"prefs.set","params":{"path":"tools.blockFingerInput","value":true}}');
      await this.wait(2000);
      native.command('{"id":"document.inspect"}');
      const driver = Driver.create();
      await delegator.executeShellCommand('uitest screenCap -p /data/local/tmp/photocraft-pinch-before.png');
      for (let repeat = 0; repeat < 20; repeat++) {
        const points = PointerMatrix.create(2, 10);
        for (let step = 0; step < 10; step++) {
          points.setPoint(0, step, { x: 1200 - (repeat % 2 === 0 ? step : 9 - step) * 15, y: 1000 });
          points.setPoint(1, step, { x: 1400 + (repeat % 2 === 0 ? step : 9 - step) * 15, y: 1000 });
        }
        const ok = await driver.injectMultiPointerAction(points, 1000);
        if (!ok) throw new Error('Multi-pointer injection failed');
        await driver.swipe(1300, 1000, 1330, 1020, 1000);
        await this.wait(100);
      }
      native.command('{"id":"document.inspect"}');
      // Replay the problematic callback ordering through the real native worker.
      // UiTest cannot force firmware to emit an axis/mouse echo after touchscreen UP.
      for (let repeat = 0; repeat < 20; repeat++) {
        native.input('{"kind":"touch","points":[{"id":1,"x":1300,"y":1000}]}');
        native.input('{"kind":"axis","action":2,"zoom":1.01,"x":1300,"y":1000}');
        await this.wait(50);
        native.input('{"kind":"touch","points":[{"id":1,"x":1200,"y":1000},{"id":2,"x":1400,"y":1000}]}');
        await this.wait(50);
        // Lift either finger first; keep the other down longer than the 300 ms guard.
        const remaining = repeat % 2 === 0 ?
          '{"kind":"touch","points":[{"id":1,"x":1200,"y":1000}]}' :
          '{"kind":"touch","points":[{"id":2,"x":1400,"y":1000}]}';
        native.input(remaining);
        if (repeat % 4 >= 2) {
          // A transient empty snapshot must not promote the re-reported remnant.
          native.input('{"kind":"touch","points":[]}');
          await this.wait(50);
          native.input(remaining);
        }
        await this.wait(700);
        native.input(repeat % 2 === 0 ?
          '{"kind":"touch","points":[{"id":1,"x":1230,"y":1010}]}' :
          '{"kind":"touch","points":[{"id":2,"x":1430,"y":1010}]}');
        native.input('{"kind":"axis","action":2,"zoom":1.01,"x":1300,"y":1000}');
        native.input('{"kind":"pointer","action":0,"x":1300,"y":1000}');
        await this.wait(100);
        native.input('{"kind":"touch","points":[]}');
        native.input('{"kind":"axis","action":3,"zoom":1.01,"x":1300,"y":1000}');
        native.input('{"kind":"pointer","action":0,"x":1300,"y":1000}');
        native.input('{"kind":"pointer","action":2,"x":1330,"y":1020}');
        native.input('{"kind":"pointer","action":1,"x":1330,"y":1020}');
        await this.wait(350);
      }
      native.command('{"id":"document.inspect"}');
      await this.wait(1000);
      await delegator.executeShellCommand('uitest screenCap -p /data/local/tmp/photocraft-pinch-after.png');
      // Recenter after navigation so the positive-control stroke lands on pixels.
      native.command('{"id":"view.fitOnScreen"}');
      await this.wait(500);
      const pen = PointerMatrix.create(1, 3);
      pen.setPoint(0, 0, { x: 1300, y: 1000 });
      pen.setPoint(0, 1, { x: 1320, y: 1010 });
      pen.setPoint(0, 2, { x: 1340, y: 1020 });
      await driver.injectPenPointerAction(pen, 1000, 1);
      await this.wait(1000);
      native.command('{"id":"document.inspect"}');
      await this.wait(1000);
      await delegator.executeShellCommand('uitest screenCap -p /data/local/tmp/photocraft-pinch-pen.png');
      await delegator.finishTest('Gestures and staggered-release replays complete; verify document revisions with scripts/test_pinch_simulator.py', 0);
    } catch (error) {
      await delegator.finishTest(`Pinch test failed: ${String(error)}`, 1);
    }
  }
}
