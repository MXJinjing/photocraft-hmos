//! Read authorized platform files without an ArkTS-sized buffer or a fixed file-size cap.
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::fd::{BorrowedFd, RawFd};

/// # Safety
/// `fd` must remain a valid, open descriptor until this function returns.
pub unsafe fn duplicate(fd: RawFd) -> Result<File, String> {
    if fd < 0 {
        return Err("Invalid file descriptor".into());
    }
    // SAFETY: The platform keeps its File open during this call. Only the duplicate
    // is retained, so the platform may close its original immediately afterwards.
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    borrowed
        .try_clone_to_owned()
        .map(File::from)
        .map_err(|e| format!("Cannot retain opened file: {e}"))
}

pub fn read(file: File) -> Result<Vec<u8>, String> {
    let size = file
        .metadata()
        .map_err(|e| format!("Cannot stat file: {e}"))?
        .len();
    read_sized(file, size)
}

fn read_sized(mut reader: impl Read, size: u64) -> Result<Vec<u8>, String> {
    if size == 0 {
        return Err("File is empty".into());
    }
    let size = usize::try_from(size).map_err(|_| "File exceeds the addressable memory size")?;
    let mut bytes = Vec::new();
    // Fail normally on capacity/allocation errors instead of aborting in read_to_end.
    // Reserve once to avoid reallocating and copying an already large input buffer.
    bytes
        .try_reserve_exact(size)
        .map_err(|e| format!("Not enough memory to open file: {e}"))?;
    let mut chunk = [0u8; 64 * 1024];
    while bytes.len() < size {
        let wanted = chunk.len().min(size - bytes.len());
        let count = match reader.read(&mut chunk[..wanted]) {
            Ok(0) => return Err("Incomplete file read".into()),
            Ok(count) => count,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("Cannot read file: {e}")),
        };
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "../tests/unit/file_open.rs"]
mod tests;
