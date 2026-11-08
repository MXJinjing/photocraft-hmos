//! A sandbox path can stop resolving after startup while its verified backing is live.
//! Separate test executable: the spill manager and host directory are process-wide.
#[cfg(unix)]
#[test]
fn open_time_capacity_uses_the_live_backing_after_sandbox_path_disappears() {
    use photocraft_raster::spill;
    let root = std::env::temp_dir().join(format!("photocraft-host-namespace-{}", std::process::id()));
    let dir = root.join("scratch");
    let moved = root.join("renamed-cache");
    spill::set_host_scratch_dir(dir.clone()).unwrap();
    spill::configure(Some(&dir), 1 << 20).unwrap();
    let initial = spill::scratch_capacity().expect("startup measures the sandbox volume");
    assert!(initial > 0);
    // Reproduce a now-unresolvable configured path, without deleting any file. The Unix
    // backing is unlinked already and remains open. Path statvfs/canonicalize now fails.
    std::fs::rename(&dir, &moved).unwrap();
    assert!(std::fs::canonicalize(&dir).is_err());
    let open_time = spill::scratch_capacity().expect("admission measures the actual backing fd");
    assert!(open_time > 0);
    assert!(!spill::free_space_unmeasured());
    assert!(spill::stats().volume_space.is_some());
    // Fresh fd measurement must still obey an explicit cap; disabling never earns credit.
    spill::set_size_cap(Some(64 << 20));
    assert!(spill::scratch_capacity().unwrap() <= (64 << 20) - (1 << 20));
    spill::configure(None, 0).unwrap();
    assert_eq!(spill::scratch_capacity(), None);
}
