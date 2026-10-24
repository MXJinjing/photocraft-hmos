import { abilityDelegatorRegistry } from '@kit.TestKit';
import native from 'libphotocraft.so';

async function wait(ms: number): Promise<void> {
  await new Promise<void>((resolve) => { setTimeout(resolve, ms); });
}

function contact(x: number, y: number): void {
  native.input(`{"kind":"touch","points":[{"id":1,"x":${x},"y":${y}}]}`);
}

async function capture(name: string): Promise<void> {
  await wait(800);
  await abilityDelegatorRegistry.getAbilityDelegator().executeShellCommand(
    `uitest screenCap -p /data/local/tmp/palm-${name}.png`);
}

// Replays the bridge's non-pen CANCEL packet through the actual native worker.
// This does not simulate palm hardware or the OS-to-XComponent callback itself.
export async function runTouchCancelTest(): Promise<void> {
  const delegator = abilityDelegatorRegistry.getAbilityDelegator();
  try {
    await delegator.startAbility({ bundleName: abilityDelegatorRegistry.getArguments().bundleName,
      moduleName: 'entry', abilityName: 'EntryAbility' });
    await wait(12000);
    native.command('{"id":"prefs.set","params":{"path":"tools.blockFingerInput","value":false}}');
    native.command('{"id":"tools.select.brush"}');
    native.command('{"id":"tools.setBrush","params":{"brush":{"size":12}}}');
    for (const mode of ['normal', 'cancel']) {
      native.command('{"id":"file.new","params":{"width":1024,"height":768}}');
      native.command('{"id":"view.fitOnScreen"}');
      await wait(1000);
      await capture(`${mode}-before`);
      contact(1500, 1100);
      await wait(150);
      contact(1580, 1140);
      await wait(200);
      await capture(`${mode}-held`);
      native.input(mode === 'normal' ? '{"kind":"touch","points":[]}' : '{"kind":"blur"}');
      await capture(`${mode}-after`);
      native.command('{"id":"document.inspect"}');
      contact(1500, 1280);
      await wait(150);
      contact(1580, 1280);
      await wait(200);
      native.input('{"kind":"touch","points":[]}');
      await capture(`${mode}-resumed`);
      native.command('{"id":"document.inspect"}');
    }
    await wait(500);
    await delegator.finishTest('Touch cancellation and next-stroke comparison captured', 0);
  } catch (error) {
    await delegator.finishTest(`Touch cancellation replay failed: ${String(error)}`, 1);
  }
}
