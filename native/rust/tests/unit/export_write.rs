use super::*;

#[test]
fn private_and_relative_writes_keep_atomic_replacement() {
    for path in ["/sandbox/recovery/doc.pcraft", "exports/image.png"] {
        assert!(
            publish(Path::new("/sandbox"), Path::new(path), b"data", |_, _| {
                panic!("private writes must not enter the bridge")
            })
            .is_none()
        );
    }
}

#[test]
fn public_exports_wait_for_publication_and_preserve_errors() {
    let path = Path::new("/Download/app/processed/image.jpg");
    let result = publish(Path::new("/sandbox"), path, b"encoded", |name, bytes| {
        assert_eq!(name, path.to_str().unwrap());
        assert_eq!(bytes, b"encoded");
        Err("publish failed".into())
    })
    .unwrap();
    assert!(result.unwrap_err().to_string().contains("publish failed"));
    assert!(
        publish(Path::new("/sandbox"), path, b"encoded", |_, _| Ok(()))
            .unwrap()
            .is_ok()
    );
}
