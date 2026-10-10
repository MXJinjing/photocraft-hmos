//! UI-language preferences, queried once without subprocesses or registry parsing.
//!
//! Native builds read `PHOTOCRAFT_LOCALE`, then the OS UI-language list. The web build reads
//! `?lang=<tag>` the same way, then `navigator.languages` (and `navigator.language`).

use super::{Lang, lang_from_tag};

static HOST_TAGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Supply the platform locale before creating the editor on a custom native host.
pub fn set_host_locale(tag: &str) {
    let _ = HOST_TAGS.set(bounded_tags([tag.to_owned()]));
}

const MAX_SYSTEM_TAGS: usize = 64;
const MAX_SYSTEM_TAG_BYTES: usize = 128;

/// Resolve Auto against the current registry. Cache the OS's tags, rather than a language,
/// so matching remains separate from platform detection.
/// Missing or unsupported system preferences always resolve to English.
pub fn system_lang() -> Lang {
    #[cfg(test)]
    {
        TEST_SYSTEM_TAGS.with(|tags| resolve(&tags.borrow()))
    }
    #[cfg(not(test))]
    {
        static SYSTEM: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        resolve(SYSTEM.get_or_init(detect_system_tags))
    }
}

fn resolve(tags: &[String]) -> Lang {
    tags.iter().find_map(|tag| lang_from_tag(tag)).unwrap_or(Lang::EN)
}

fn bounded_tags(tags: impl IntoIterator<Item = String>) -> Vec<String> {
    tags.into_iter()
        .take(MAX_SYSTEM_TAGS)
        .filter(|tag| {
            !tag.trim().is_empty()
                && tag.len() <= MAX_SYSTEM_TAG_BYTES
                && tag.trim().bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'@'))
        })
        .map(|tag| tag.trim().to_owned())
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_system_tags() -> Vec<String> {
    if let Some(tags) = HOST_TAGS.get() {
        return tags.clone();
    }
    if let Ok(tag) = std::env::var("PHOTOCRAFT_LOCALE")
        && !tag.trim().is_empty()
    {
        return bounded_tags([tag]);
    }
    // sys-locale exposes safe wrappers for GetUserPreferredUILanguages (Windows) and
    // CFLocaleCopyPreferredLanguages (macOS); Unix uses its standard locale environment.
    // A failure in platform interop must also leave the app usable in English.
    std::panic::catch_unwind(|| bounded_tags(sys_locale::get_locales())).unwrap_or_default()
}

/// `lang` from a page search string (`?webgl&cpu&lang=zh-Hans`). An absent or empty value is not an
/// override, so the caller can follow the browser list. The first non-empty `lang` wins.
#[cfg(any(test, target_arch = "wasm32"))]
fn query_lang(search: &str) -> Option<String> {
    let search = match search.split_once('#') {
        Some((before, _)) => before,
        None => search,
    };
    let search = search.strip_prefix('?').unwrap_or(search);
    if search.is_empty() {
        return None;
    }
    for pair in search.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if !key.eq_ignore_ascii_case("lang") {
            continue;
        }
        let trimmed = percent_decode(value).trim().to_owned();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

#[cfg(any(test, target_arch = "wasm32"))]
fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decode `%HH` sequences. `+` stays literal: language tags are not form-urlencoded, and
/// `encodeURIComponent` uses `%20` for spaces.
#[cfg(any(test, target_arch = "wasm32"))]
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let percent = bytes.get(i) == Some(&b'%');
        let hi = i.checked_add(1).and_then(|n| bytes.get(n)).copied().and_then(from_hex);
        let lo = i.checked_add(2).and_then(|n| bytes.get(n)).copied().and_then(from_hex);
        if percent && let (Some(hi), Some(lo)) = (hi, lo) {
            out.push((hi << 4) | lo);
            i = i.saturating_add(3);
            continue;
        }
        if let Some(byte) = bytes.get(i) {
            out.push(*byte);
        }
        i = i.saturating_add(1);
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Auto's tag list for a web launch. A non-empty `?lang=` is a single-tag override, like
/// `PHOTOCRAFT_LOCALE`; otherwise `navigator` is the browser's preferred order.
#[cfg(any(test, target_arch = "wasm32"))]
fn locale_tags_from_browser(search: &str, navigator: impl IntoIterator<Item = String>) -> Vec<String> {
    if let Some(tag) = query_lang(search) {
        return bounded_tags([tag]);
    }
    bounded_tags(navigator)
}

#[cfg(all(not(test), target_arch = "wasm32"))]
fn page_search() -> String {
    web_sys::window().and_then(|window| window.location().search().ok()).unwrap_or_default()
}

#[cfg(all(not(test), target_arch = "wasm32"))]
fn navigator_language_tags() -> Vec<String> {
    let Some(window) = web_sys::window() else { return Vec::new() };
    let navigator = window.navigator();
    let listed = navigator.languages();
    let mut tags = Vec::new();
    let count = usize::try_from(listed.length()).unwrap_or(0).min(MAX_SYSTEM_TAGS);
    for index in 0..count {
        let Some(index) = u32::try_from(index).ok() else { break };
        if let Some(tag) = listed.get(index).as_string() {
            tags.push(tag);
        }
    }
    if tags.is_empty()
        && let Some(tag) = navigator.language()
    {
        tags.push(tag);
    }
    tags
}

#[cfg(all(not(test), target_arch = "wasm32"))]
fn detect_system_tags() -> Vec<String> {
    // wasm32 aborts on panic, so this path only uses Option/Result host getters.
    let tags = locale_tags_from_browser(&page_search(), navigator_language_tags());
    log::info!("web UI language tags: {tags:?}");
    tags
}

#[cfg(test)]
thread_local! {
    // Keep ordinary tests independent of the developer's locale, without mutating OS or
    // process-global preferences. Startup tests supply the OS result on their own thread.
    static TEST_SYSTEM_TAGS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(super) fn with_system_tags<R>(tags: &[&str], run: impl FnOnce() -> R) -> R {
    struct Restore(Vec<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_SYSTEM_TAGS.set(std::mem::take(&mut self.0));
        }
    }
    let _restore = Restore(TEST_SYSTEM_TAGS.replace(tags.iter().map(|tag| (*tag).to_owned()).collect()));
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_preferences_are_negotiated_in_order() {
        for (tags, expected) in [
            (vec!["ja-JP", "en-US"], "ja"),
            (vec!["fr-CA", "en-US"], "fr"),
            (vec!["uk-UA", "en-US"], "uk"),
            (vec!["sv-SE", "uk_UA.UTF-8", "ru-RU"], "uk"),
            (vec!["sv-SE", "ko-KR", "fr-FR"], "ko"),
            (vec!["zh-Hant-HK", "zh-CN"], "zh-hant"),
            (vec!["zh-Hans-CN", "zh-TW"], "zh-hans"),
            (vec!["en-US", "ru-RU"], "en"),
            (vec!["sv-SE", "ar-SA"], "en"),
            (vec!["de-AT", "en-US"], "de"),
            (vec!["pt-PT"], "pt-br"),
            (vec!["it-IT", "en-US"], "it"),
            (vec!["nl-BE", "fr-BE"], "nl"),
            (vec![], "en"),
        ] {
            with_system_tags(&tags, || assert_eq!(system_lang().code(), expected));
        }
    }

    #[test]
    fn test_system_preferences_restore_and_remain_thread_local() {
        with_system_tags(&["ko-KR"], || {
            with_system_tags(&["fr-FR"], || assert_eq!(system_lang().code(), "fr"));
            assert_eq!(system_lang().code(), "ko");
            std::thread::spawn(|| assert_eq!(system_lang(), Lang::EN)).join().expect("locale test thread");
            assert!(std::panic::catch_unwind(|| with_system_tags(&["ja-JP"], || panic!("test unwind"))).is_err());
            assert_eq!(system_lang().code(), "ko");
        });
        assert_eq!(system_lang(), Lang::EN);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_locale_results_are_bounded_and_trimmed() {
        assert_eq!(bounded_tags(["  fr-CA  ".into(), "".into(), " ".into(), "fr-\0".into(), "x".repeat(129)]), ["fr-CA"]);
        assert_eq!(bounded_tags(std::iter::repeat_n("en-US".into(), 100)).len(), MAX_SYSTEM_TAGS);
        let detected = detect_system_tags();
        assert!(detected.len() <= MAX_SYSTEM_TAGS);
        assert!(detected.iter().all(|tag| !tag.is_empty() && tag.len() <= MAX_SYSTEM_TAG_BYTES));
    }

    #[test]
    fn web_lang_query_overrides_navigator_and_resolves_chinese() {
        // The HarmonyOS wrapper appends the system locale to the existing web flags.
        for search in ["?webgl&cpu&lang=zh-Hans-CN", "?webgl&cpu&lang=zh-Hans", "?lang=zh-CN", "?lang=zh_CN.UTF-8", "?LANG=zh-hans"] {
            let tags = locale_tags_from_browser(search, ["en-US".into(), "en".into()]);
            assert_eq!(resolve(&tags).code(), "zh-hans", "{search}");
        }
        let traditional = locale_tags_from_browser("?lang=zh-Hant-TW", ["zh-CN".into()]);
        assert_eq!(resolve(&traditional).code(), "zh-hant");
    }

    #[test]
    fn web_without_lang_follows_navigator_order() {
        let tags = locale_tags_from_browser("?webgl&cpu", ["sv-SE".into(), "zh-Hant-HK".into(), "en-US".into()]);
        assert_eq!(tags, ["sv-SE", "zh-Hant-HK", "en-US"]);
        assert_eq!(resolve(&tags).code(), "zh-hant");
        assert_eq!(locale_tags_from_browser("", ["ja-JP".into()]), ["ja-JP"]);
        assert_eq!(locale_tags_from_browser("?lang=", ["fr-FR".into()]), ["fr-FR"]);
        assert_eq!(locale_tags_from_browser("?lang=%20", ["ko-KR".into()]), ["ko-KR"]);
    }

    #[test]
    fn unsupported_lang_override_does_not_fall_through() {
        let tags = locale_tags_from_browser("?lang=sv-SE", ["ja-JP".into()]);
        assert_eq!(tags, ["sv-SE"]);
        assert_eq!(resolve(&tags), Lang::EN);
        assert!(locale_tags_from_browser("?lang=zh-Hans\0", ["ja-JP".into()]).is_empty());
        assert!(locale_tags_from_browser(&format!("?lang={}", "x".repeat(200)), ["ja-JP".into()]).is_empty());
    }

    #[test]
    fn lang_query_decodes_and_ignores_a_fragment() {
        assert_eq!(query_lang("?webgl&lang=zh%2DHans&cpu"), Some("zh-Hans".into()));
        assert_eq!(query_lang("?lang=%20ja-JP%20#ignored"), Some("ja-JP".into()));
        assert_eq!(query_lang("?cpu&lang=&lang=pt-BR"), Some("pt-BR".into()));
        assert_eq!(query_lang("?lang"), None);
        assert_eq!(query_lang("?webgl&cpu"), None);
    }
}
