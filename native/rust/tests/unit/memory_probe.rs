use super::*;

#[test]
fn describe_reports_probe_fields_for_device_logs() {
    let line = describe(SystemMemory { available_ram: Some(3 << 30), swap_free: None, commit_headroom: None, constraint_unreadable: true });
    for key in ["available_ram=Some(3221225472)", "constraint_unreadable=true", "hard_limit=", "scratch_credit(", "hard_limit_with_scratch="] {
        assert!(line.contains(key), "{key} missing from {line}");
    }
}

#[test]
fn unreadable_probe_with_measured_memory_admits_reference_psd() {
    let m = SystemMemory { available_ram: Some(3 << 30), swap_free: None, commit_headroom: None, constraint_unreadable: true };
    let b = resolve_budget_with_scratch(m.into(), None, REFERENCE_ESTIMATE);
    assert!(b.hard_limit >= REFERENCE_ESTIMATE, "{b:?}");
}

#[test]
fn genuinely_low_memory_still_refuses_reference_psd() {
    let m = SystemMemory { available_ram: Some(32 << 20), swap_free: None, commit_headroom: None, constraint_unreadable: false };
    let b = resolve_budget_with_scratch(m.into(), None, 0);
    assert!(b.hard_limit < REFERENCE_ESTIMATE, "{b:?}");
}

#[test]
fn probe_reports_scratch_state_after_host_configuration() {
    let line = describe(SystemMemory { available_ram: Some(6 << 30), ..Default::default() });
    for key in ["scratch_dir=", "scratch_capacity=", "scratch_budget=", "scratch_backing_verified=", "scratch_paused=", "scratch_unmeasured=", "scratch_written=", "scratch_error="] {
        assert!(line.contains(key), "{line}");
    }
}
