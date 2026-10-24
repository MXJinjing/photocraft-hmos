#!/usr/bin/env python3
"""Compare touch lift/cancellation and the next stroke on a 2880 x 1920 Pad emulator.

Requires Pillow. Install a separate .palmtest application and entry_test HAP built
with TouchCancelTest first. Replays native adapter packets, not physical palms.
--phase before --verify-existing also validates the original six-image baseline.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
from PIL import Image, ImageChops

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--hdc', default='/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc')
parser.add_argument('--target', default='127.0.0.1:5555')
parser.add_argument('--bundle', default='io.github.storytold.photocraft.hmos.palmtest')
parser.add_argument('--output-dir', type=Path, required=True)
parser.add_argument('--phase', choices=('before', 'after'), default='after')
parser.add_argument('--verify-existing', action='store_true')
args = parser.parse_args()
args.output_dir.mkdir(parents=True, exist_ok=True)


def run(*command, timeout=120):
    result = subprocess.run([args.hdc, '-t', args.target, *command], check=True,
                            capture_output=True, text=True, timeout=timeout)
    return result.stdout + result.stderr


if not args.verify_existing:
    if run('shell', 'param', 'get', 'const.product.model').strip().lower() != 'emulator' or not args.bundle.endswith('.palmtest'):
        raise SystemExit('Use an emulator and an independent .palmtest bundle.')
    run('shell', 'aa', 'force-stop', args.bundle)
    run('shell', 'uitest', 'start-daemon', 'default')
    result = run('shell', 'aa', 'test', '-b', args.bundle, '-m', 'entry_test',
                 '-s', 'unittest', 'OpenHarmonyTestRunner', '-s', 'touchCancel', 'true',
                 '-s', 'timeout', '60000', timeout=90)
    (args.output_dir / 'test-result.log').write_text(result)
    if 'TestFinished-ResultCode: 0' not in result:
        raise SystemExit(result)
    logs = run('shell', 'hilog', '-x')
    commands = [line for line in logs.splitlines() if 'PhotoCraft native command: file.new: Ok' in line]
    if not commands:
        raise SystemExit('Missing native document creation.')
    pid = commands[-1].split()[2]
    (args.output_dir / 'native.log').write_text(run('shell', 'hilog', '-x', '-P', pid))
    for case in ('normal', 'cancel'):
        for stage in ('before', 'held', 'after', 'resumed'):
            name = f'{case}-{stage}'
            run('file', 'recv', f'/data/local/tmp/palm-{name}.png', str(args.output_dir / f'{name}.png'))


# Fixed fixture viewport. Excludes chrome and leaves an 8 px inset inside canvas.
# Coordinates are physical screenshot pixels, including the 78 px status bar.
def ink(name):
    im = Image.open(args.output_dir / f'{name}.png').convert('RGB')
    if im.size != (2880, 1920):
        raise SystemExit(f'Unexpected screenshot size: {im.size}')
    r, g, b = im.crop((650, 665, 1655, 1415)).split()
    return ImageChops.lighter(ImageChops.lighter(r, g), b).point(lambda v: 255 if v < 80 else 0)


def count(mask):
    return mask.histogram()[255]


for case in ('normal', 'cancel'):
    if count(ink(f'{case}-before')) > 10:
        raise SystemExit(f'Fixture canvas is not blank: {case}')
    if count(ink(f'{case}-held')) < 300:
        raise SystemExit(f'Missing intended stroke: {case}')
normal, cancelled = ink('normal-after'), ink('cancel-after')
extra = count(ImageChops.subtract(cancelled, normal))
missing = count(ImageChops.subtract(normal, cancelled))
report = {'normal_ink_pixels': count(normal), 'cancel_ink_pixels': count(cancelled),
          'extra_cancel_pixels': extra, 'missing_cancel_pixels': missing}
if args.phase == 'before':
    if extra < 1000:
        raise SystemExit(f'Baseline did not reproduce the extra segment: {report}')
else:
    if extra > 20 or missing > 20:
        raise SystemExit(f'Cancelled stroke differs from normal lift: {report}')
    for case in ('normal', 'cancel'):
        before, after = ink(f'{case}-after'), ink(f'{case}-resumed')
        added = count(ImageChops.subtract(after, before))
        if added < 300 or count(ImageChops.subtract(before, after)) > 20:
            raise SystemExit(f'Next stroke missing or previous ink lost: {case}, {added}')
        report[f'{case}_next_stroke_pixels'] = added
    if count(ImageChops.difference(ink('normal-resumed'), ink('cancel-resumed'))) > 40:
        raise SystemExit('Next stroke after cancellation differs from the control.')
    logs = (args.output_dir / 'native.log').read_text()
    inspections = [line for line in logs.splitlines() if 'PhotoCraft native command: document.inspect: Ok' in line]
    revisions = [int(v) for line in inspections for v in re.findall(r'"revision": Number\((\d+)\)', line)]
    if len(revisions) != 4 or revisions[1] <= revisions[0] or revisions[3] <= revisions[2]:
        raise SystemExit(f'Expected two committed strokes per document: {revisions}')
    report['revisions'] = revisions
(args.output_dir / 'pixels.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'PASS ({args.phase}): {report}')
