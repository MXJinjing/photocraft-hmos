//! A platform save dialog that finishes after [`crate::Services::write`] returns.
//!
//! On the web, Save clicks a download link and returns immediately. The HarmonyOS wrapper then
//! shows the system save picker. Quitting in that gap kills the picker, so the unsaved-changes
//! prompt waits on this gate until the wrapper reports the picker's result.

use std::sync::{Arc, Mutex, PoisonError};

/// What the platform save dialog has done for one armed write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveConfirm {
    /// `write` did not open a dialog for this attempt.
    NotStarted,
    /// The dialog is still open.
    Pending,
    /// The dialog closed. `true` when the file was written.
    Finished(bool),
}

/// Hooks the unsaved-changes prompt polls. Installed only when a host shows a save picker after
/// `write` (the HarmonyOS wrapper).
pub struct SaveDialog {
    arm: Box<dyn FnMut() -> u64>,
    disarm: Box<dyn FnMut()>,
    poll: Box<dyn FnMut(u64) -> SaveConfirm>,
}

impl SaveDialog {
    pub fn new(arm: impl FnMut() -> u64 + 'static, disarm: impl FnMut() + 'static, poll: impl FnMut(u64) -> SaveConfirm + 'static) -> Self {
        Self { arm: Box::new(arm), disarm: Box::new(disarm), poll: Box::new(poll) }
    }

    pub(crate) fn arm(&mut self) -> u64 {
        (self.arm)()
    }

    pub(crate) fn disarm(&mut self) {
        (self.disarm)();
    }

    pub(crate) fn poll(&mut self, ticket: u64) -> SaveConfirm {
        (self.poll)(ticket)
    }
}

#[derive(Default)]
struct Inner {
    next: u64,
    /// Generation passed to the next `write`, or 0 when nothing is armed.
    armed: u64,
    pending: u64,
    pending_name: Option<String>,
    result_gen: u64,
    result_saved: bool,
}

/// Shared by the web download and the save-dialog hooks. One armed save at a time; a later
/// download (Export As) does not complete an earlier quit-save, and a stale picker result does
/// not complete the next one.
pub struct HostSaveGate {
    inner: Mutex<Inner>,
}

impl HostSaveGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { inner: Mutex::new(Inner::default()) })
    }

    pub fn dialog(self: &Arc<Self>) -> SaveDialog {
        let arm_gate = Arc::clone(self);
        let disarm_gate = Arc::clone(self);
        let poll_gate = Arc::clone(self);
        SaveDialog::new(move || arm_gate.arm(), move || disarm_gate.disarm(), move |ticket| poll_gate.poll(ticket))
    }

    pub fn arm(&self) -> u64 {
        let mut g = self.lock();
        g.next = g.next.saturating_add(1).max(1);
        g.armed = g.next;
        g.next
    }

    pub fn disarm(&self) {
        self.lock().armed = 0;
    }

    /// If a save dialog was armed, this download is the one it is waiting on.
    pub fn note_if_armed(&self, name: &str) -> bool {
        let mut g = self.lock();
        if g.armed == 0 {
            return false;
        }
        g.pending = g.armed;
        g.pending_name = Some(name.to_string());
        g.armed = 0;
        true
    }

    /// Record the picker result for the armed download named `name`. Returns whether it matched
    /// that download (an unrelated export must not release a quit that is still saving).
    pub fn finish(&self, name: &str, saved: bool) -> bool {
        let mut g = self.lock();
        if g.pending == 0 {
            return false;
        }
        if !name.is_empty() && !names_match(g.pending_name.as_deref().unwrap_or(""), name) {
            return false;
        }
        let ticket = g.pending;
        g.pending = 0;
        g.pending_name = None;
        g.result_gen = ticket;
        g.result_saved = saved;
        true
    }

    pub fn poll(&self, ticket: u64) -> SaveConfirm {
        if ticket == 0 {
            return SaveConfirm::NotStarted;
        }
        let mut g = self.lock();
        if g.result_gen == ticket {
            let saved = g.result_saved;
            g.result_gen = 0;
            return SaveConfirm::Finished(saved);
        }
        if g.pending == ticket {
            return SaveConfirm::Pending;
        }
        SaveConfirm::NotStarted
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// ArkWeb may replace characters the save picker rejects; the file is still the one we armed.
fn names_match(pending: &str, event: &str) -> bool {
    if pending == event {
        return true;
    }
    squash_name(pending) == squash_name(event)
}

fn squash_name(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    file.chars().map(|c| if matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_result_is_ignored_until_the_armed_download_finishes() {
        let gate = HostSaveGate::new();
        let ticket = gate.arm();
        assert_eq!(gate.poll(ticket), SaveConfirm::NotStarted, "write has not started");
        gate.note_if_armed("Untitled.psd");
        assert_eq!(gate.poll(ticket), SaveConfirm::Pending);
        assert!(!gate.finish("other.png", true), "a different export must not release this save");
        assert_eq!(gate.poll(ticket), SaveConfirm::Pending);
        assert!(gate.finish("Untitled.psd", false));
        assert_eq!(gate.poll(ticket), SaveConfirm::Finished(false));
        assert_eq!(gate.poll(ticket), SaveConfirm::NotStarted, "the result is consumed once");
    }

    #[test]
    fn a_stale_result_does_not_complete_the_next_save() {
        let gate = HostSaveGate::new();
        let first = gate.arm();
        gate.note_if_armed("A.psd");
        gate.disarm();
        let second = gate.arm();
        gate.note_if_armed("B.psd");
        assert!(!gate.finish("A.psd", true));
        assert_eq!(gate.poll(first), SaveConfirm::NotStarted);
        assert_eq!(gate.poll(second), SaveConfirm::Pending);
        assert!(gate.finish("B.psd", true));
        assert_eq!(gate.poll(second), SaveConfirm::Finished(true));
    }

    #[test]
    fn disarm_drops_an_arm_no_write_consumed() {
        let gate = HostSaveGate::new();
        let ticket = gate.arm();
        gate.disarm();
        gate.note_if_armed("Untitled.psd");
        assert_eq!(gate.poll(ticket), SaveConfirm::NotStarted);
    }
}
