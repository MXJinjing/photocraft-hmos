//! The platform waits only on the Rust worker, never on the ArkTS main thread.
use std::collections::HashMap;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};

pub type Notify = extern "C" fn(u64, *const u8, usize, *const u8, usize);
static NOTIFY: OnceLock<Notify> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
static CANCELLED: AtomicBool = AtomicBool::new(false);
static PENDING: OnceLock<Mutex<HashMap<u64, mpsc::Sender<Reply>>>> = OnceLock::new();
pub type Reply = Result<(String, Vec<u8>), String>;
fn pending() -> &'static Mutex<HashMap<u64, mpsc::Sender<Reply>>> {
    PENDING.get_or_init(Default::default)
}
pub fn init(notify: Notify) {
    let _ = NOTIFY.set(notify);
}
pub fn notify(id: u64, kind: &str, text: &str, data: &[u8]) {
    if let Some(f) = NOTIFY.get() {
        let message = serde_json::json!({"kind": kind, "text": text}).to_string();
        // The C++ callback copies both borrowed slices before it returns.
        f(
            id,
            message.as_ptr(),
            message.len(),
            data.as_ptr(),
            data.len(),
        );
    }
}
pub fn request(kind: &str, text: &str, data: &[u8]) -> Reply {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    {
        let mut map = pending().lock().unwrap_or_else(|p| p.into_inner());
        if CANCELLED.load(Ordering::Acquire) {
            return Err("cancelled".into());
        }
        map.insert(id, tx);
    }
    notify(id, kind, text, data);
    rx.recv()
        .unwrap_or_else(|_| Err("platform service disconnected".into()))
}
pub fn reply(id: u64, result: Reply) {
    if let Some(tx) = pending()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&id)
    {
        let _ = tx.send(result);
    }
}
pub fn cancel() {
    let mut map = pending().lock().unwrap_or_else(|p| p.into_inner());
    CANCELLED.store(true, Ordering::Release);
    for (_, tx) in map.drain() {
        let _ = tx.send(Err("cancelled".into()));
    }
}
pub fn resume() {
    CANCELLED.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_wakes_waiters_and_rejects_new_requests() {
        let (tx, rx) = mpsc::channel();
        pending().lock().unwrap().insert(42, tx);
        cancel();
        assert_eq!(rx.recv().unwrap(), Err("cancelled".into()));
        assert_eq!(request("save", "x.psd", &[]), Err("cancelled".into()));
        reply(42, Ok(("late".into(), vec![]))); // Late picker completions are ignored.
        resume();
    }
}
