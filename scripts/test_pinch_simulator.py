#!/usr/bin/env python3
"""Run the installed UiTest HAP on an emulator and assert native document revisions."""
import argparse
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--hdc', default='/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc')
parser.add_argument('--target', default='127.0.0.1:5555')
parser.add_argument('--bundle', default='io.github.storytold.photocraft.pinchtest')
parser.add_argument('--output-dir', type=Path, default=Path('work/pinch-test'))
args = parser.parse_args()
args.output_dir.mkdir(parents=True, exist_ok=True)

def run(*command, timeout=120):
    result = subprocess.run([args.hdc, '-t', args.target, *command], check=True,
                            capture_output=True, text=True, timeout=timeout)
    return result.stdout + result.stderr

model = run('shell', 'param', 'get', 'const.product.model').strip()
if model.lower() != 'emulator' or not args.bundle.endswith('.pinchtest'):
    raise SystemExit('Use an emulator and an independent .pinchtest bundle.')
run('shell', 'uitest', 'start-daemon', 'default')
result = run('shell', 'aa', 'test', '-b', args.bundle, '-m', 'entry_test',
             '-s', 'unittest', 'OpenHarmonyTestRunner', '-s', 'timeout', '120000', timeout=150)
(args.output_dir / 'test-result.log').write_text(result)
if 'TestFinished-ResultCode: 0' not in result:
    raise SystemExit(result)
logs = run('shell', 'hilog', '-x')
new_doc = [line for line in logs.splitlines() if 'PhotoCraft native command: file.new: Ok' in line]
if not new_doc:
    raise SystemExit('No new-document notification in simulator logs.')
pid = new_doc[-1].split()[2]
process_logs = run('shell', 'hilog', '-x', '-P', pid)
(args.output_dir / 'native.log').write_text(process_logs)
revisions = [int(value) for value in re.findall(r'"revision": Number\((\d+)\)', process_logs)]
if len(revisions) != 4 or revisions[:3] != [revisions[0]] * 3 or revisions[3] <= revisions[0]:
    raise SystemExit(f'Unexpected document revisions: {revisions}')
if '"tools.blockFingerInput": Bool(true)' not in process_logs:
    raise SystemExit('Finger blocking was not enabled.')
for phase in ('before', 'after', 'pen'):
    run('file', 'recv', f'/data/local/tmp/photocraft-pinch-{phase}.png',
        str(args.output_dir / f'{phase}.png'))
print(f'PASS: 20 OS pinch gestures, 20 staggered-release/echo replays; revisions {revisions}; pen paints.')
