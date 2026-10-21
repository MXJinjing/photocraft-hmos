use super::*;
use serde_json::json;

// One test owns the process-wide roots, so the checks don't race each other.
#[test]
fn only_sandbox_and_granted_paths_pass() {
    init(&[
        "/data/app/files",
        "/storage/Users/currentUser/Download/com.x",
    ]);
    assert!(check("/data/app/files/imports/a.png").is_ok());
    assert!(check("/storage/Users/currentUser/Download/com.x/a.psd").is_ok());
    assert!(check("relative/a.png").is_ok());
    assert!(
        check("save-as/3/a.png").is_ok(),
        "picker tokens are relative"
    );
    assert!(check("").is_ok());
    let e = check("/storage/Users/currentUser/Desktop/a.png").unwrap_err();
    assert!(e.contains("Choose"), "{e}");
    assert!(
        check("/data/app/files/../other").is_err(),
        "no climbing out of a root"
    );
    assert!(check("../escape").is_err());
    assert!(check("a/../../escape").is_err());
    assert!(
        check("/data/app/filesystem").is_err(),
        "a sibling with the same prefix"
    );

    grant("/storage/Users/currentUser/Desktop/batch");
    assert!(check("/storage/Users/currentUser/Desktop/batch/in/1.jpg").is_ok());
    assert!(check("/storage/Users/currentUser/Desktop/other").is_err());

    // Params: only path-like keys, at any depth, strings or lists of strings.
    assert!(check_params(&json!({"input": "/data/app/files/in", "output": "/storage/Users/currentUser/Desktop/batch/out"})).is_ok());
    assert!(check_params(&json!({"name": "/not/a/path/key", "layer": {"name": "/x"}})).is_ok());
    assert!(check_params(&json!({"paths": ["/data/app/files/a", "/etc/passwd"]})).is_err());
    assert!(
        check_params(
            &json!({"value": {"plugIns": {"additionalPluginsFolder": "/sdcard/plugins"}}})
        )
        .is_err()
    );
    assert!(check_params(&json!({"steps": [{"params": {"path": "/sdcard/x.psd"}}]})).is_err());
    let e = gate("file.open", &json!({"path": "/sdcard/x.psd"}))
        .unwrap_err()
        .to_string();
    assert!(e.contains("file.open"), "{e}");
    assert!(gate("file.open", &json!({"path": "/data/app/files/x.psd"})).is_ok());
}
