//! A sandbox cache override must back real, lossless disk eviction, not just admission credit.
use photocraft_color::PixelFormat;
use photocraft_geom::TileCoord;
use photocraft_raster::{Surface, spill};

#[test]
fn host_cache_override_spills_reloads_and_fails_closed() {
    assert!(spill::host_scratch_dir().is_none());
    assert!(spill::set_host_scratch_dir("relative-cache".into()).is_err());
    let dir = std::env::temp_dir().join(format!("photocraft-host-cache-{}", std::process::id())).join("photocraft-scratch");
    spill::set_host_scratch_dir(dir.clone()).unwrap();
    spill::set_host_scratch_dir(dir.clone()).unwrap();
    assert!(spill::set_host_scratch_dir(dir.join("different")).is_err());
    assert_eq!(spill::host_scratch_dir(), Some(dir.clone()));
    spill::configure(spill::host_scratch_dir().as_deref(), 1).unwrap();
    assert!(spill::scratch_capacity().is_some_and(|n| n > 0));
    let mut pixels = Surface::new(PixelFormat::RGBA8);
    pixels.tile_mut(TileCoord::new(0, 0)).bytes_mut().fill(73);
    spill::settle();
    let before = spill::stats();
    assert!(before.evictions > 0 && before.written_bytes > 0 && before.scratch_bytes > 0, "{before:?}");
    assert_eq!(before.resident_bytes, 0);
    let file = spill::scratch_handle_for_tests().unwrap();
    assert!(file.metadata().unwrap().len() > 0, "bytes really reached a disk file");
    assert!(pixels.tile(TileCoord::new(0, 0)).unwrap().try_bytes().unwrap().iter().all(|&b| b == 73));
    assert!(spill::stats().reloads > before.reloads);
    spill::set_size_cap(Some(1));
    assert!(spill::scratch_capacity().is_none_or(|n| n <= 1));
    spill::configure(None, 0).unwrap();
    assert_eq!(spill::scratch_capacity(), None, "disabled disk earns no credit");
    // A file instead of a directory is unwritable: no promised room even with a size cap.
    let blocked = dir.join("not-a-directory");
    std::fs::write(&blocked, b"x").unwrap();
    assert!(spill::configure(Some(&blocked), 1).is_err());
    assert_eq!(spill::scratch_capacity(), None);
    spill::configure(None, 0).unwrap();
}
