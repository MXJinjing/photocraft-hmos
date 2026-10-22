#!/usr/bin/env python3
"""Compare stationary Free Transform pixels in an independent Pad emulator HAP.

Replays native adapter input through the OHOS worker. This tests the editor's
handling of stationary contact reports, not physical touchscreen firmware.
Requires Pillow and a fixed 2880 x 1920 emulator.
"""
import argparse
from pathlib import Path
import subprocess
import re
from PIL import Image, ImageChops

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--hdc', default='/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc')
parser.add_argument('--target', default='127.0.0.1:5555')
parser.add_argument('--bundle', default='io.github.mxjinjing.photocraft.transformtest')
parser.add_argument('--phase', choices=('before', 'after'), default='after')
parser.add_argument('--output-dir', type=Path, required=True)
parser.add_argument('--verify-existing', action='store_true')
args = parser.parse_args()
args.output_dir.mkdir(parents=True, exist_ok=True)


def run(*command, timeout=120):
    result = subprocess.run([args.hdc, '-t', args.target, *command], check=True,
                            capture_output=True, text=True, timeout=timeout)
    return result.stdout + result.stderr


cases = [f'{source}-{motion}' for source in ('touch', 'pen', 'mouse') for motion in ('still', 'moved')]
if not args.verify_existing:
    if run('shell', 'param', 'get', 'const.product.model').strip().lower() != 'emulator' or not args.bundle.endswith('.transformtest'):
        raise SystemExit('Use an emulator and an independent .transformtest bundle.')
    run('shell', 'aa', 'force-stop', args.bundle)
    run('shell', 'uitest', 'start-daemon', 'default')
    result = run('shell', 'aa', 'test', '-b', args.bundle, '-m', 'entry_test',
                 '-s', 'unittest', 'OpenHarmonyTestRunner', '-s', 'transformTouch', 'true',
                 '-s', 'timeout', '120000', timeout=150)
    (args.output_dir / 'test-result.log').write_text(result)
    if 'TestFinished-ResultCode: 0' not in result:
        raise SystemExit(result)
    logs = run('shell', 'hilog', '-x')
    commands = [line for line in logs.splitlines() if 'PhotoCraft native command: file.new: Ok' in line]
    if not commands:
        raise SystemExit('Missing fixture creation in native log.')
    pid = commands[-1].split()[2]
    (args.output_dir / 'native.log').write_text(run('shell', 'hilog', '-x', '-P', pid))
    for case in cases:
        for stage in ('before', 'held', 'after'):
            run('file', 'recv', f'/data/local/tmp/photocraft-transform-{case}-{stage}.png',
                str(args.output_dir / f'{case}-{stage}.png'))


def red_mask(path):
    im = Image.open(path).convert('RGB')
    if im.size != (2880, 1920):
        raise SystemExit(f'Unexpected screenshot size: {im.size}')
    # Compare the red fixture, excluding toolbar text, cursors and marching ants.
    r, g, b = im.split()
    mask = ImageChops.multiply(r.point(lambda v: 255 if v > 180 else 0),
                              ImageChops.multiply(g.point(lambda v: 255 if v < 80 else 0),
                                                  b.point(lambda v: 255 if v < 80 else 0)))
    if sum(mask.histogram()[1:]) < 1000:
        raise SystemExit(f'Missing red transform fixture: {path}')
    return mask


reports = [line for line in (args.output_dir / 'native.log').read_text().splitlines()
           if 'PhotoCraft native command: edit.freeTransform: Ok(Object' in line]
quads = []
for line in reports:
    quad = re.search(r'"quad": Array \[(.*?)\], "rect"', line)
    if quad:
        quads.append(tuple(float(v) for v in re.findall(r'Number\(([^)]+)\)', quad[1])))
if len(quads) != len(cases) * 4 or any(len(q) != 8 for q in quads):
    raise SystemExit(f'Expected four transform reports per case, got {len(quads)}')

changes = {}
for index, case in enumerate(cases):
    _, before, held, after = quads[index * 4:index * 4 + 4]
    if case.endswith('still') and (before != held or held != after):
        raise SystemExit(f'Stationary contact changed transform geometry: {case}')
    masks = {stage: red_mask(args.output_dir / f'{case}-{stage}.png') for stage in ('before', 'held', 'after')}
    diff = ImageChops.difference(masks['held'], masks['after'])
    changes[case] = sum(diff.histogram()[1:])
    if case.endswith('moved') and (before == held or sum(ImageChops.difference(masks['before'], masks['held']).histogram()[1:]) < 100):
        raise SystemExit(f'Drag did not change the fixture: {case}')
    if args.phase == 'before' and case == 'touch-moved':
        if changes[case] < 100 or held == after:
            raise SystemExit(f'Baseline did not reproduce drift: {changes}')
    elif case.endswith('moved') and (changes[case] > 10 or held != after):
        raise SystemExit(f'Stationary contact changed transform pixels: {changes}')
moved_changes = {case: count for case, count in changes.items() if case.endswith('moved')}
print(f'PASS ({args.phase}): changed red pixels after movement stopped: {moved_changes}; all stationary-only geometry unchanged (long press may open a context menu).')
