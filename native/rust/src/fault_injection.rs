//! Explicit opt-in simulator hook. Not present in production builds.
use std::sync::atomic::{AtomicBool, Ordering};
static PANIC: AtomicBool = AtomicBool::new(false);
pub fn request() {
    PANIC.store(true, Ordering::Release);
}
pub fn check() {
    if PANIC.swap(false, Ordering::AcqRel) {
        std::panic::resume_unwind(Box::new("injected native UI rendering panic".to_owned()));
    }
}
