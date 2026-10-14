//! Recovery policy shared by the worker's error and panic paths.
use std::time::Instant;

#[derive(Default)]
pub struct Recovery {
    pub halted: bool,
}
impl Recovery {
    pub fn failed(&mut self) {
        self.halted = true;
    }
    pub fn frame_deadline(&self, requested: Instant) -> Instant {
        requested
    }
}

pub fn stop_gpu(app: &mut photocraft_ui_egui::PhotocraftApp, reason: &str) {
    // The native host deliberately stops after a GPU failure. Restarting the app
    // is the recovery operation, so do not switch the upstream canvas to CPU.
    app.perf.gpu = false;
    app.perf.gpu_info.canvas = "stopped".into();
    app.perf.gpu_info.selected = "stopped".into();
    app.perf.gpu_info.backend = "stopped".into();
    app.perf.gpu_info.adapter = "GPU rendering stopped".into();
    app.perf.gpu_info.device_type = "unknown".into();
    app.perf.gpu_info.fallback = Some(reason.into());
    app.perf.gpu_fallback = Some(reason.into());
    app.ui.status = format!("GPU rendering stopped: {reason}");
    app.ui.status_error = true;
}

pub fn finish_interrupted_pass(
    ctx: &egui::Context,
) -> Result<(), String> {
    let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ctx.end_pass()))
        .map_err(|_| "could not finish the interrupted egui pass")?;
    output.drop_without_applying_deltas();
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/recovery.rs"]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests;
