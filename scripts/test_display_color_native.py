"""Check SDK color-space aliases without needing a device or linked NDK library."""
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
sdk = Path(os.environ.get('PHOTOCRAFT_NATIVE_SDK',
    '/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native'))
with tempfile.TemporaryDirectory() as tmp:
    # Expose only the standalone SDK enum header, keeping host system headers intact.
    headers = Path(tmp) / 'native_buffer'
    headers.mkdir()
    (headers / 'buffer_common.h').symlink_to(sdk / 'sysroot/usr/include/native_buffer/buffer_common.h')
    source = Path(tmp) / 'test.cpp'
    source.write_text('''
#include "display_color.h"
#include <cassert>
int main() {
    assert(PhotoCraftOutputMatches(false, OH_COLORSPACE_SRGB_FULL));
    assert(PhotoCraftOutputMatches(false, OH_COLORSPACE_DISPLAY_SRGB));
    assert(PhotoCraftOutputMatches(true, OH_COLORSPACE_P3_FULL));
    assert(PhotoCraftOutputMatches(true, OH_COLORSPACE_DISPLAY_P3_SRGB));
    assert(!PhotoCraftOutputMatches(true, OH_COLORSPACE_SRGB_FULL));
    assert(!PhotoCraftOutputMatches(false, OH_COLORSPACE_P3_FULL));
    assert(!PhotoCraftOutputMatches(true, OH_COLORSPACE_P3_PQ_FULL));
    assert(!PhotoCraftOutputMatches(true, OH_COLORSPACE_P3_HLG_FULL));
    assert(!PhotoCraftOutputMatches(true, OH_COLORSPACE_P3_LIMIT));
    assert(!PhotoCraftOutputMatches(false, OH_COLORSPACE_NONE));
}
''')
    output = Path(tmp) / 'test'
    subprocess.run(['clang++', '-std=c++17', '-I', str(root / 'entry/src/main/cpp'),
        '-I', tmp, str(source), '-o', str(output)], check=True)
    subprocess.run([str(output)], check=True)
print('Passed: sRGB/P3 aliases accepted; mismatched, untagged, limited and HDR outputs rejected.')
