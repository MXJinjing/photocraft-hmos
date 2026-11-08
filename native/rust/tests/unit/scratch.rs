use super::*;

#[test]
fn scratch_uses_private_cache_or_legacy_files_fallback() {
    assert_eq!(
        directory("/sandbox/cache", "/sandbox/files").unwrap(),
        Path::new("/sandbox/cache/photocraft-scratch")
    );
    assert_eq!(
        directory("", "/sandbox/files").unwrap(),
        Path::new("/sandbox/files/cache/photocraft-scratch")
    );
}

#[test]
fn scratch_refuses_relative_or_empty_roots() {
    assert!(directory("relative", "/sandbox/files").is_err());
    assert!(directory("", "").is_err());
}
