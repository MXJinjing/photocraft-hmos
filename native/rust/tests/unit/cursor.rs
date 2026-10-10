use super::*;

// Logic runs before UI: preserve the cursor chosen by the later canvas/panels pass.
// PlatformOutput::append otherwise replaces it with the logic pass's default arrow.
fn append_logic(output: &mut egui::PlatformOutput, logic: egui::PlatformOutput) {
    let icon = output.cursor_icon;
    let image = output.cursor_image.take();
    output.append(logic);
    output.cursor_icon = icon;
    output.cursor_image = image;
}

#[test]
fn ui_cursor_survives_merging_the_earlier_logic_pass() {
    for icon in [
        egui::CursorIcon::None,
        egui::CursorIcon::Crosshair,
        egui::CursorIcon::Default,
    ] {
        let mut ui = egui::PlatformOutput {
            cursor_icon: icon,
            ..Default::default()
        };
        let logic = egui::PlatformOutput {
            commands: vec![egui::OutputCommand::CopyText("test".into())],
            ..Default::default()
        };
        append_logic(&mut ui, logic);
        assert_eq!(ui.cursor_icon, icon);
        assert_eq!(ui.commands.len(), 1);
    }
}

#[test]
fn cursor_hides_for_tip_and_restores_for_chrome_and_after_resume() {
    let mut sync = CursorSync::default();
    for icon in [
        egui::CursorIcon::Default,
        egui::CursorIcon::None,
        egui::CursorIcon::Crosshair,
        egui::CursorIcon::Grabbing,
        egui::CursorIcon::Default,
    ] {
        assert_eq!(sync.update(icon), Some(format!("{icon:?}")));
        assert_eq!(sync.update(icon), None);
    }
    sync.invalidate();
    assert_eq!(
        sync.update(egui::CursorIcon::Default).as_deref(),
        Some("Default")
    );
}
