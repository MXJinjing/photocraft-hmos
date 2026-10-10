use super::*;
#[test]
fn viewing_allocates_no_target_and_first_saves_keep_separate_targets() {
    let mut docs = Documents::new("/Download/test.bundle");
    assert!(docs.target(1).is_none());
    assert!(docs.target(2).is_none());
    docs.select(1, "/Download/test.bundle/画.psd").unwrap();
    docs.select(2, "/Download/test.bundle/画1.psd").unwrap();
    assert!(docs.target(3).is_none());
    assert!(docs.target(4).is_none());
    docs.select(3, "/Download/test.bundle/photo.psd").unwrap();
    assert_eq!(
        docs.target(3).as_deref(),
        Some("/Download/test.bundle/photo.psd")
    );
    assert!(docs.select(3, "/elsewhere/画.psd").is_err());
    assert!(
        docs.select(3, "/Download/test.bundle/../outside.psd")
            .is_err()
    );
    assert_eq!(
        docs.target(1).as_deref(),
        Some("/Download/test.bundle/画.psd")
    );
}
