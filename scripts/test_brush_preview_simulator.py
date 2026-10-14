#!/usr/bin/env python3
"""Verify brush preview pixels and document revisions in an independent emulator HAP.

Requires Pillow. The ohosTest runner replays pen/mouse/touch input through the
real native worker; it does not emulate firmware pen proximity callbacks.
"""
import argparse
from pathlib import Path
import re
import subprocess
from PIL import Image, ImageChops

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--hdc', default='/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc')
parser.add_argument('--target', default='127.0.0.1:5555')
parser.add_argument('--bundle', default='io.github.storytold.photocraft.brushpreviewtest')
parser.add_argument('--phase', choices=('before', 'after'), default='after')
parser.add_argument('--surface-offset-y', type=int, default=78,
                    help='XComponent top inset in the fixed API 20 emulator screenshot')
parser.add_argument('--output-dir', type=Path, default=Path('work/brush-preview/after'))
parser.add_argument('--verify-existing', action='store_true')
args = parser.parse_args()
args.output_dir.mkdir(parents=True, exist_ok=True)

def run(*command, timeout=120):
    result = subprocess.run([args.hdc, '-t', args.target, *command], check=True,
                            capture_output=True, text=True, timeout=timeout)
    return result.stdout + result.stderr

phases = ('blank', 'hover', 'leave', 'stationary', 'resume', 'lift',
          'lift-stationary', 'lift-resume', 'mouse', 'touch')
if not args.verify_existing:
    model = run('shell', 'param', 'get', 'const.product.model').strip()
    if model.lower() != 'emulator' or not args.bundle.endswith('.brushpreviewtest'):
        raise SystemExit('Use an emulator and an independent .brushpreviewtest bundle.')
    run('shell', 'aa', 'force-stop', args.bundle)
    run('shell', 'uitest', 'start-daemon', 'default')
    result = run('shell', 'aa', 'test', '-b', args.bundle, '-m', 'entry_test',
                 '-s', 'unittest', 'OpenHarmonyTestRunner', '-s', 'brushPreview', 'true',
                 '-s', 'timeout', '60000', timeout=90)
    (args.output_dir / 'test-result.log').write_text(result)
    if 'TestFinished-ResultCode: 0' not in result:
        raise SystemExit(result)
    logs = run('shell', 'hilog', '-x')
    new_doc = [line for line in logs.splitlines() if 'PhotoCraft native command: file.new: Ok' in line]
    if not new_doc:
        raise SystemExit('No new-document notification in simulator logs.')
    pid = new_doc[-1].split()[2]
    (args.output_dir / 'native.log').write_text(run('shell', 'hilog', '-x', '-P', pid))
    for phase in phases:
        run('file', 'recv', f'/data/local/tmp/photocraft-brush-{phase}.png',
            str(args.output_dir / f'{phase}.png'))

logs = (args.output_dir / 'native.log').read_text()
inspections = [line for line in logs.splitlines() if 'PhotoCraft native command: document.inspect: Ok' in line]
revisions = [int(value) for line in inspections for value in re.findall(r'"revision": Number\((\d+)\)', line)]
if len(revisions) != 4 or revisions[1] != revisions[0] or revisions[2] <= revisions[1] or revisions[3] != revisions[2]:
    raise SystemExit(f'Unexpected document revisions: {revisions}')
images = {phase: Image.open(args.output_dir / f'{phase}.png').convert('RGB') for phase in phases}
if any(im.size != (2880, 1920) for im in images.values()):
    raise SystemExit('Use the fixed 2880 x 1920 Pad emulator for pixel comparisons.')

def changed(a, b, point):
    x, y = point
    # native.input coordinates are relative to the XComponent, while screenCap
    # includes the emulator's status bar above the app window.
    y += args.surface_offset_y
    crop = (x - 90, y - 90, x + 90, y + 90)
    difference = ImageChops.difference(images[a].crop(crop), images[b].crop(crop))
    pixels = difference.tobytes()
    return sum(max(pixels[i:i + 3]) > 8 for i in range(0, len(pixels), 3))

visible = {
    'pen hover': changed('blank', 'hover', (1400, 1100)),
    'hover resume': changed('blank', 'resume', (1500, 1100)),
    'lift resume': changed('lift', 'lift-resume', (1500, 1200)),
    'mouse hover': changed('touch', 'mouse', (1500, 1000)),
}
hidden = {
    'leave': changed('blank', 'leave', (1400, 1100)),
    'stationary after leave': changed('blank', 'stationary', (1400, 1100)),
    'stationary after lift': changed('lift', 'lift-stationary', (1350, 1200)),
    'touch': changed('blank', 'touch', (1500, 1000)),
}
if any(value > 5 for value in hidden.values()):
    raise SystemExit(f'Hidden preview changed canvas pixels: {hidden}')
if args.phase == 'after' and any(value < 20 for value in visible.values()):
    raise SystemExit(f'Missing visible brush preview: {visible}')
if args.phase == 'before' and any(value > 5 for value in visible.values()):
    raise SystemExit(f'Baseline unexpectedly has painted previews: {visible}')
print(f'PASS ({args.phase}): preview pixels {visible}; hidden pixels {hidden}; revisions {revisions}.')
