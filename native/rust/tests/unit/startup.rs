use super::FirstFrame;

#[test]
fn skipped_frames_do_not_reveal_the_surface() {
    let mut frame = FirstFrame::default();
    assert!(!frame.update(false));
    assert!(!frame.update(false));
    assert!(frame.update(true));
}

#[test]
fn only_the_first_successful_presentation_notifies() {
    let mut frame = FirstFrame::default();
    assert!(frame.update(true));
    assert!(!frame.update(true));
    assert!(!frame.update(false));
    assert!(!frame.update(true));
}

#[test]
fn replaced_surface_waits_for_its_own_first_frame() {
    let mut frame = FirstFrame::default();
    assert!(frame.update(true));
    frame.invalidate();
    assert!(!frame.update(false));
    assert!(frame.update(true));
}
