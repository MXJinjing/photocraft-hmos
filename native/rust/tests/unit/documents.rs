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

#[test]
fn ordinary_picker_targets_are_per_document_and_save_as_is_not_reusable() {
    let mut docs = Documents::new("/Download/test.bundle");
    docs.select(1, "save-default/10/中文.psd").unwrap();
    docs.select(2, "save-default/11/中文.psd").unwrap();
    assert_eq!(docs.target(1).as_deref(), Some("save-default/10/中文.psd"));
    assert_eq!(docs.target(2).as_deref(), Some("save-default/11/中文.psd"));
    for invalid in [
        "save-as/10/a.psd",
        "save-default/0/a.psd",
        "save-default/x/a.psd",
        "save-default/10/../a.psd",
        "save-default/10/",
        "save-default/10/..",
    ] {
        assert!(docs.select(1, invalid).is_err());
    }
    assert_eq!(docs.target(1).as_deref(), Some("save-default/10/中文.psd"));
}

#[test]
fn opened_local_psd_reuses_source_but_external_and_flat_files_need_picker() {
    let mut docs = Documents::new("/Download/test.bundle");
    assert_eq!(
        docs.existing(1, Some("/Download/test.bundle/photo.PSB"))
            .as_deref(),
        Some("/Download/test.bundle/photo.PSB")
    );
    assert!(docs.existing(2, Some("/elsewhere/photo.psd")).is_none());
    assert!(
        docs.existing(3, Some("/Download/test.bundle/photo.png"))
            .is_none()
    );
    assert!(docs.existing(4, None).is_none());
    docs.select(2, "save-default/22/picked.psd").unwrap();
    assert_eq!(
        docs.existing(2, Some("save-as/23/copy.psd")).as_deref(),
        Some("save-default/22/picked.psd")
    );
}
