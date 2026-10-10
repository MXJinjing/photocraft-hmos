use super::*;

#[test]
fn sandbox_work_supports_relative_files_in_an_isolated_process() {
    let before = std::env::current_dir().unwrap();
    let root = std::env::temp_dir().join(format!("photocraft-work-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "working_directory::tests::child_working_directory",
            "--nocapture",
        ])
        .env("PHOTOCRAFT_TEST_WORK_FILES", &root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(root.join("work").is_dir());
    assert!(!root.join("work/relative-renamed.txt").exists());
    assert_eq!(std::env::current_dir().unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn child_working_directory() {
    let Ok(root) = std::env::var("PHOTOCRAFT_TEST_WORK_FILES") else {
        return;
    };
    let work = initialize(&root).unwrap();
    assert_eq!(
        work,
        std::fs::canonicalize(Path::new(&root).join("work")).unwrap()
    );
    std::fs::write("relative.txt", b"script output").unwrap();
    assert_eq!(std::fs::read("relative.txt").unwrap(), b"script output");
    std::fs::rename("relative.txt", "relative-renamed.txt").unwrap();
    std::fs::remove_file("relative-renamed.txt").unwrap();
    assert_eq!(initialize(&root).unwrap(), work);
    let before = std::env::current_dir().unwrap();
    assert!(initialize(".").is_err());
    std::fs::write("blocked", b"file").unwrap();
    assert!(initialize(work.join("blocked").to_str().unwrap()).is_err());
    assert_eq!(std::env::current_dir().unwrap(), before);
    std::fs::remove_file("blocked").unwrap();
}
