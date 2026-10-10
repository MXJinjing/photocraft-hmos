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
    // 先登记应答通道再通知平台，避免快速应答找不到请求；等待时不持有 pending 锁。
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
    // 生命周期中断时唤醒全部等待者，防止 worker 永久卡在已关闭的系统选择器上。
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
#[path = "../tests/unit/bridge.rs"]
mod tests;
