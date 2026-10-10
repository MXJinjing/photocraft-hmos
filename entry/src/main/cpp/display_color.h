#pragma once
#include <native_buffer/buffer_common.h>

// NativeWindow's getter may normalize FULL to DISPLAY_*. Accept only equivalent
// primaries, transfer function and range; HDR and limited-range variants differ.
inline bool PhotoCraftOutputMatches(bool p3, OH_NativeBuffer_ColorSpace actual) {
    return p3 ? (actual == OH_COLORSPACE_P3_FULL || actual == OH_COLORSPACE_DISPLAY_P3_SRGB)
              : (actual == OH_COLORSPACE_SRGB_FULL || actual == OH_COLORSPACE_DISPLAY_SRGB);
}
