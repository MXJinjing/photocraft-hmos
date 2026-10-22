import { abilityDelegatorRegistry } from '@kit.TestKit';
import native from 'libphotocraft.so';

async function wait(ms: number): Promise<void> {
  await new Promise<void>((resolve) => { setTimeout(resolve, ms); });
}

function contact(source: string, action: number, x: number, y: number): void {
  if (source === 'touch') {
    native.input(action === 1 ? '{"kind":"touch","points":[]}' :
      `{"kind":"touch","points":[{"id":1,"x":${x},"y":${y}}]}`);
  } else {
    native.input(`{"kind":"pointer","source":"${source}","action":${action},"x":${x},"y":${y},"sample":{"pressure":${action === 1 ? 0 : 0.5}}}`);
  }
}

async function capture(phase: string): Promise<void> {
  native.command('{"id":"edit.freeTransform"}');
  await abilityDelegatorRegistry.getAbilityDelegator().executeShellCommand(
    `uitest screenCap -p /data/local/tmp/photocraft-transform-${phase}.png`);
}

export async function runTransformTouchTest(): Promise<void> {
  const delegator = abilityDelegatorRegistry.getAbilityDelegator();
  try {
    await delegator.startAbility({ bundleName: abilityDelegatorRegistry.getArguments().bundleName,
      moduleName: 'entry', abilityName: 'EntryAbility' });
    await wait(10000);
    native.command('{"id":"file.new","params":{"width":512,"height":512}}');
    native.command('{"id":"layer.new.layer","params":{"name":"Transform fixture"}}');
    native.command('{"id":"select.rect","params":{"x":128,"y":128,"width":256,"height":256}}');
    native.command('{"id":"edit.fill","params":{"color":"#ff0000"}}');
    native.command('{"id":"select.deselect"}');
    native.command('{"id":"tools.select.brush"}');
    native.command('{"id":"prefs.set","params":{"path":"tools.blockFingerInput","value":false}}');
    native.command('{"id":"view.actualPixels"}');
    await wait(1000);
    for (const source of ['touch', 'pen', 'mouse']) {
      for (const moved of [false, true]) {
        native.command('{"id":"edit.freeTransform"}');
        await wait(500);
        const name = `${source}-${moved ? 'moved' : 'still'}`;
        await capture(`${name}-before`);
        contact(source, 0, 1300, 1000);
        await wait(30);
        const x = moved ? 1340 : 1300;
        contact(source, 2, x, 1000);
        await wait(100);
        await capture(`${name}-held`);
        for (let i = 0; i < 60; i++) {
          contact(source, 2, x, 1000);
          await wait(40);
        }
        await capture(`${name}-after`);
        contact(source, 1, x, 1000);
        await wait(100);
        native.input('{"kind":"key","code":2070,"action":0}');
        native.input('{"kind":"key","code":2070,"action":1}');
        await wait(300);
      }
    }
    native.command('{"id":"document.inspect"}');
    await wait(300);
    await delegator.finishTest('Transform stationary-contact replay complete', 0);
  } catch (error) {
    await delegator.finishTest(`Transform touch test failed: ${String(error)}`, 1);
  }
}
