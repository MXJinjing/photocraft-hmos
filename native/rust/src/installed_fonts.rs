//! Fonts the user installed through HarmonyOS Settings > System > 字体安装和管理.
//!
//! They live outside `/system/fonts`. HarmonyOS (API 14+) lists them through `native_drawing`:
//! `OH_Drawing_GetSystemFontFullNamesByType(INSTALLED)` gives their full names and
//! `OH_Drawing_GetFontDescriptorByFullName` each one's file path. The paths are handed to the text
//! engine, which registers them like the scanned system fonts (unreadable files are skipped).

use std::path::PathBuf;

/// Decodes an `OH_Drawing_String` payload (UTF-16, native endianness).
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub(crate) fn utf16_bytes_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_ne_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// Lists the installed fonts and hands their files to the text engine. Call before the font
/// database is built.
pub(crate) fn register() -> usize {
    let paths = installed_font_paths();
    let n = paths.len();
    photocraft_text::extra_fonts::set_extra_font_paths(paths);
    n
}

#[cfg(target_env = "ohos")]
mod ffi {
    use std::ffi::{CStr, c_char, c_int};

    #[repr(C)]
    pub struct OhString {
        pub str_data: *mut u8,
        pub str_len: u32,
    }
    #[repr(C)]
    pub struct OhArray {
        _p: [u8; 0],
    }
    #[repr(C)]
    pub struct OhFontDescriptor {
        pub path: *mut c_char,
        pub post_script_name: *mut c_char,
        pub full_name: *mut c_char,
        pub font_family: *mut c_char,
        pub font_subfamily: *mut c_char,
        pub weight: c_int,
        pub width: c_int,
        pub italic: c_int,
        pub mono_space: bool,
        pub symbolic: bool,
    }
    /// `OH_Drawing_SystemFontType::INSTALLED`.
    pub const INSTALLED: c_int = 1 << 3;

    #[link(name = "native_drawing")]
    unsafe extern "C" {
        pub fn OH_Drawing_GetSystemFontFullNamesByType(font_type: c_int) -> *mut OhArray;
        pub fn OH_Drawing_GetDrawingArraySize(arr: *mut OhArray) -> usize;
        pub fn OH_Drawing_GetSystemFontFullNameByIndex(
            arr: *mut OhArray,
            index: usize,
        ) -> *const OhString;
        pub fn OH_Drawing_DestroySystemFontFullNames(arr: *mut OhArray);
        pub fn OH_Drawing_GetFontDescriptorByFullName(
            full_name: *const OhString,
            font_type: c_int,
        ) -> *mut OhFontDescriptor;
        pub fn OH_Drawing_DestroyFontDescriptor(d: *mut OhFontDescriptor);
    }

    pub fn cstr(p: *const c_char) -> Option<String> {
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

/// File paths of the user-installed fonts that the app can read. Empty off OHOS.
pub(crate) fn installed_font_paths() -> Vec<PathBuf> {
    #[cfg(target_env = "ohos")]
    {
        use ffi::*;
        let mut paths = Vec::new();
        // SAFETY: documented native_drawing calls; the array and each descriptor are freed with
        // their matching destroy functions, and the names are borrowed from the array.
        unsafe {
            let arr = OH_Drawing_GetSystemFontFullNamesByType(INSTALLED);
            if arr.is_null() {
                return paths;
            }
            for i in 0..OH_Drawing_GetDrawingArraySize(arr) {
                let name = OH_Drawing_GetSystemFontFullNameByIndex(arr, i);
                if name.is_null() {
                    continue;
                }
                let d = OH_Drawing_GetFontDescriptorByFullName(name, INSTALLED);
                if d.is_null() {
                    let n = &*name;
                    let label = if n.str_data.is_null() {
                        String::new()
                    } else {
                        utf16_bytes_to_string(std::slice::from_raw_parts(
                            n.str_data,
                            n.str_len as usize,
                        ))
                    };
                    eprintln!("photocraft-text: no descriptor for installed font {label:?}");
                    continue;
                }
                if let Some(p) = cstr((*d).path).filter(|p| !p.is_empty()) {
                    paths.push(PathBuf::from(p));
                }
                OH_Drawing_DestroyFontDescriptor(d);
            }
            OH_Drawing_DestroySystemFontFullNames(arr);
        }
        paths
    }
    #[cfg(not(target_env = "ohos"))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_decode() {
        let bytes: Vec<u8> = "思源黑体 Bold"
            .encode_utf16()
            .flat_map(u16::to_ne_bytes)
            .collect();
        assert_eq!(utf16_bytes_to_string(&bytes), "思源黑体 Bold");
    }

    #[test]
    fn no_installed_fonts_off_ohos() {
        if !cfg!(target_env = "ohos") {
            assert!(installed_font_paths().is_empty());
        }
    }
}
