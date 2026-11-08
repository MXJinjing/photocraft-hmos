//! Logs the memory probe and the PSD admission budget it yields, so a device log (hilog,
//! domain 0xD001 tag "PhotoCraft") shows why a document was admitted or refused.

use photocraft_io::memory::{MemorySnapshot, resolve_budget_with_scratch};
use photocraft_raster::memory::system::{self, SystemMemory};

/// Reference estimate: the small PSD reported as refused on device (162 MiB).
const REFERENCE_ESTIMATE: u64 = 170_134_172;

/// One-line summary of a probe reading and the budgets it implies.
pub(crate) fn describe(m: SystemMemory) -> String {
    let snap: MemorySnapshot = m.into();
    let plain = resolve_budget_with_scratch(snap, None, 0);
    let spill = resolve_budget_with_scratch(snap, None, REFERENCE_ESTIMATE);
    let credit = spill.hard_limit.saturating_sub(plain.hard_limit);
    let capacity = photocraft_raster::spill::scratch_capacity();
    let stats = photocraft_raster::spill::stats();
    format!(
        "memory probe: available_ram={:?} swap_free={:?} constraint_unreadable={} hard_limit={} soft_limit={} scratch_credit({}B)={} hard_limit_with_scratch={} scratch_dir={:?} scratch_capacity={:?} scratch_budget={} scratch_backing_verified={} scratch_paused={} scratch_unmeasured={} scratch_written={} scratch_error={:?}",
        m.available_ram, m.swap_free, m.constraint_unreadable, plain.hard_limit, plain.soft_limit, REFERENCE_ESTIMATE, credit, spill.hard_limit,
        stats.dir, capacity, stats.budget_bytes, cfg!(target_env = "ohos") && stats.budget_bytes > 0 && !stats.paused, stats.paused, stats.free_space_unmeasured, stats.written_bytes, stats.last_error
    )
}

/// Reads the probe and logs it.
pub(crate) fn log_current() {
    log(&describe(system::current()));
}

#[cfg(target_env = "ohos")]
fn log(line: &str) {
    use std::ffi::{CString, c_char, c_int, c_uint};
    unsafe extern "C" {
        fn OH_LOG_Print(kind: c_int, level: c_int, domain: c_uint, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
    }
    let Ok(text) = CString::new(line) else { return };
    // LOG_APP = 0, LOG_INFO = 4; same domain and tag as bridge.cpp.
    unsafe { OH_LOG_Print(0, 4, 0xD001, c"PhotoCraft".as_ptr(), c"%{public}s".as_ptr(), text.as_ptr()) };
}

#[cfg(not(target_env = "ohos"))]
fn log(line: &str) {
    eprintln!("photocraft-hmos: {line}");
}

#[cfg(test)]
#[path = "../tests/unit/memory_probe.rs"]
mod tests;
