use super::*;
static TEST_LOCK: Mutex<()> = Mutex::new(());
#[test]
fn cancellation_wakes_waiters_and_rejects_new_requests() {
    let _guard = TEST_LOCK.lock().unwrap();
    let (tx, rx) = mpsc::channel();
    pending().lock().unwrap().insert(42, tx);
    cancel();
    assert_eq!(rx.recv().unwrap(), Err("cancelled".into()));
    assert_eq!(request("save", "x.psd", &[]), Err("cancelled".into()));
    reply(42, Ok(("late".into(), vec![]))); // Late picker completions are ignored.
    resume();
}

#[test]
fn invalid_replies_wake_waiters_and_file_reply_retains_original_descriptor() {
    use std::io::{Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;
    use std::time::Duration;
    let _guard = TEST_LOCK.lock().unwrap();
    resume();
    for id in 50..53 {
        let (tx, rx) = mpsc::channel();
        pending().lock().unwrap().insert(id, tx);
        unsafe {
            match id {
                50 => crate::pc_reply(id, true, std::ptr::null(), 1, std::ptr::null(), 0),
                51 => crate::pc_reply(id, true, b"x".as_ptr(), 1, std::ptr::null(), 1),
                _ => crate::pc_reply_file(id, b"x".as_ptr(), 1, -1),
            }
        }
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().is_err());
    }
    let path = std::env::temp_dir().join(format!("photocraft-reply-file-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(path).unwrap();
    file.write_all(b"file reply").unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let (tx, rx) = mpsc::channel();
    pending().lock().unwrap().insert(53, tx);
    unsafe { crate::pc_reply_file(53, b"x.psd".as_ptr(), 5, file.as_raw_fd()) };
    drop(file);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        Ok(("x.psd".into(), b"file reply".to_vec()))
    );
}
