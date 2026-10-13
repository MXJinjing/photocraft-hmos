import { abilityDelegatorRegistry } from '@kit.TestKit';
import native from 'libphotocraft.so';

async function wait(ms: number): Promise<void> {
  await new Promise<void>((resolve) => { setTimeout(resolve, ms); });
}

function pen(action: number, x: number, y: number, pressure: number): void {
  native.input(`{"kind":"pointer","source":"pen","action":${action},"x":${x},"y":${y},"sample":{"pressure":${pressure},"tiltX":0,"tiltY":0,"eraser":false}}`);
}

async function capture(phase: string): Promise<void> {
  await wait(700);
  await abilityDelegatorRegistry.getAbilityDelegator().executeShellCommand(
    `uitest screenCap -p /data/local/tmp/photocraft-brush-${phase}.png`);
}

// Test HAP only: replay input through the real OHOS worker and capture rendered pixels.
export async function runBrushPreviewTest(): Promise<void> {
  const delegator = abilityDelegatorRegistry.getAbilityDelegator();
  try {
    await delegator.startAbility({ bundleName: abilityDelegatorRegistry.getArguments().bundleName,
      moduleName: 'entry', abilityName: 'EntryAbility' });
    await wait(15000);
    native.command('{"id":"file.new","params":{"width":512,"height":512}}');
    native.command('{"id":"tools.setBrush","params":{"brush":{"size":24}}}');
    native.command('{"id":"view.fitOnScreen"}');
    await wait(1000);
    native.command('{"id":"document.inspect"}');
    await capture('blank');
    pen(2, 1300, 1000, 0);
    pen(2, 1400, 1100, 0);
    await capture('hover');
    native.input('{"kind":"hover_leave"}');
    await capture('leave');
    // Cached/stationary reports must remain hidden, then real movement resumes.
    pen(2, 1400, 1100, 0);
    await capture('stationary');
    pen(2, 1500, 1100, 0);
    await capture('resume');
    native.command('{"id":"document.inspect"}');
    pen(0, 1300, 1200, 0.5);
    await wait(100);
    pen(2, 1350, 1200, 0.5);
    await wait(100);
    pen(1, 1350, 1200, 0);
    pen(2, 1350, 1200, 0);
    await capture('lift');
    pen(2, 1350, 1200, 0);
    await capture('lift-stationary');
    pen(2, 1500, 1200, 0);
    await capture('lift-resume');
    native.command('{"id":"document.inspect"}');
    native.input('{"kind":"pointer","action":2,"x":1500,"y":1000}');
    await capture('mouse');
    native.input('{"kind":"pointer","source":"touch","action":2,"x":1500,"y":1000}');
    await capture('touch');
    native.command('{"id":"document.inspect"}');
    await wait(300);
    await delegator.finishTest('Brush preview replay complete; verify pixels and revisions with scripts/test_brush_preview_simulator.py', 0);
  } catch (error) {
    await delegator.finishTest(`Brush preview test failed: ${String(error)}`, 1);
  }
}
