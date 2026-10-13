use super::*;
use std::io::{Cursor, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;

#[test]
fn empty_truncated_and_unaddressable_files_return_errors() {
    assert_eq!(read_sized(Cursor::new([]), 0), Err("File is empty".into()));
    assert_eq!(
        read_sized(Cursor::new([1, 2]), 3),
        Err("Incomplete file read".into())
    );
    assert!(
        read_sized(Cursor::new([]), u64::MAX)
            .unwrap_err()
            .contains("memory")
    );
    assert!(unsafe { duplicate(-1) }.is_err());
}

#[test]
fn short_reads_and_interrupted_reads_preserve_every_byte() {
    struct ShortReader {
        data: Cursor<Vec<u8>>,
        interrupted: bool,
    }
    impl Read for ShortReader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(std::io::Error::from(ErrorKind::Interrupted));
            }
            let len = out.len().min(3);
            self.data.read(&mut out[..len])
        }
    }
    let expected: Vec<u8> = (0..17).collect();
    let reader = ShortReader {
        data: Cursor::new(expected.clone()),
        interrupted: false,
    };
    assert_eq!(read_sized(reader, expected.len() as u64).unwrap(), expected);
}

#[test]
fn progress_reports_start_mib_boundaries_and_completion() {
    let size = 2 * 1024 * 1024 + 17;
    let data = vec![7u8; size];
    let mut progress = Vec::new();
    let bytes = read_sized_with_progress(Cursor::new(data), size as u64, &mut |read, total| {
        progress.push((read, total));
    })
    .unwrap();
    assert_eq!(bytes.len(), size);
    assert_eq!(progress.first(), Some(&(0, size as u64)));
    assert_eq!(progress.last(), Some(&(size as u64, size as u64)));
    assert!(progress.iter().any(|(read, _)| *read >= 1024 * 1024));
    assert!(progress.iter().all(|(_, total)| *total == size as u64));
}

#[test]
fn io_errors_are_reported_without_partial_success() {
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(ErrorKind::PermissionDenied))
        }
    }
    assert!(
        read_sized(BrokenReader, 1)
            .unwrap_err()
            .starts_with("Cannot read file:")
    );
}

#[test]
fn retained_descriptor_opens_a_valid_image_larger_than_256_mib_after_original_close() {
    // A small valid BMP with a sparse trailing region isolates the input-byte limit
    // from decoder/pixel limits, without committing a large fixture to the repo.
    let path =
        std::env::temp_dir().join(format!("photocraft-large-open-{}.bmp", std::process::id()));
    let mut original = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let size = 300 * 1024 * 1024;
    let mut header = vec![0u8; 54];
    header[..2].copy_from_slice(b"BM");
    header[2..6].copy_from_slice(&(size as u32).to_le_bytes());
    header[10..14].copy_from_slice(&54u32.to_le_bytes());
    header[14..18].copy_from_slice(&40u32.to_le_bytes());
    header[18..22].copy_from_slice(&2u32.to_le_bytes());
    header[22..26].copy_from_slice(&2u32.to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes());
    header[28..30].copy_from_slice(&24u16.to_le_bytes());
    header[34..38].copy_from_slice(&16u32.to_le_bytes());
    original.write_all(&header).unwrap();
    original
        .write_all(&[0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0])
        .unwrap();
    original.set_len(size).unwrap();
    original.seek(SeekFrom::Start(0)).unwrap();
    let retained = unsafe { duplicate(original.as_raw_fd()) }.unwrap();
    drop(original); // Matches ArkTS closing its File immediately after NAPI handoff.
    let bytes = read(retained).unwrap();
    assert_eq!(bytes.len(), size as usize);
    assert_eq!(
        unsafe { crate::bytes(bytes.as_ptr(), bytes.len()) }
            .unwrap()
            .len(),
        size as usize
    );
    assert_eq!(&bytes[..2], b"BM");
    let imported = photocraft_io::import("large.bmp", &bytes).unwrap();
    assert_eq!(imported.document.size.width, 2);
    assert_eq!(imported.document.size.height, 2);
}
