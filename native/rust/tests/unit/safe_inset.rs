use super::*;

#[test]
fn bottom_points_converts_pixels_and_rejects_bad_values() {
    assert_eq!(bottom_points(60.0, 2.0, 800.0), 30.0);
    assert_eq!(bottom_points(-5.0, 2.0, 800.0), 0.0);
    assert_eq!(bottom_points(60.0, 0.0, 800.0), 0.0);
    assert_eq!(bottom_points(f32::NAN, 2.0, 800.0), 0.0);
    assert_eq!(bottom_points(60.0, f32::INFINITY, 800.0), 0.0);
    // UI zoom enters through the scale, so the same pixels become fewer points when zoomed in.
    assert_eq!(bottom_points(60.0, 3.0, 800.0), 20.0);
}

#[test]
fn bottom_points_never_takes_more_than_a_quarter_of_the_window() {
    assert_eq!(bottom_points(10_000.0, 1.0, 800.0), 200.0);
    assert_eq!(bottom_points(10_000.0, 1.0, 0.0), 0.0);
}

fn frame(ctx: &egui::Context, app: &mut PhotocraftApp, bottom: Option<f32>) -> egui::FullOutput {
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280., 800.),
        )),
        safe_area_insets: bottom.map(insets),
        ..Default::default()
    };
    crate::editor_frame(ctx, app, raw)
}

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), Default::default())
}

fn fills(output: &egui::FullOutput, color: Color32) -> Vec<Rect> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) if rect.fill == color => Some(rect.rect),
            _ => None,
        })
        .collect()
}

#[test]
fn layout_stays_above_the_inset_and_the_host_fills_the_strip() {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut app = app();
    // Fonts become available one pass after setup; settle before measuring.
    for _ in 0..3 {
        frame(&ctx, &mut app, None).drop_without_applying_deltas();
    }
    assert!(app.ui.panels.status_bar);
    let chrome = photocraft_ui_egui::theme::Tokens::get(&ctx).chrome;

    let plain = frame(&ctx, &mut app, Some(0.0));
    assert_eq!(ctx.content_rect().bottom(), 800.0);
    let bottoms: Vec<f32> = fills(&plain, chrome).iter().map(|r| r.bottom()).collect();
    assert!(
        bottoms.contains(&800.0),
        "status bar reaches the edge: {bottoms:?}"
    );
    plain.drop_without_applying_deltas();

    let inset = frame(&ctx, &mut app, Some(24.0));
    assert_eq!(ctx.content_rect().bottom(), 776.0);
    assert_eq!(ctx.viewport_rect().bottom(), 800.0);
    let rects = fills(&inset, chrome);
    // The host strip, plus the status bar now ending at the inset instead of the screen edge.
    assert!(
        rects
            .iter()
            .any(|r| r.top() == 776.0 && r.bottom() == 800.0 && r.width() == 1280.0),
        "{rects:?}"
    );
    assert!(rects.iter().any(|r| r.bottom() == 776.0), "{rects:?}");
    // Nothing else (panels, central panel) starts inside the inset; shapes wholly off screen
    // (the editor parks some measuring widgets there) do not count.
    let other_below = inset
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) if (776.0..800.0).contains(&rect.rect.top()) => Some(rect.rect),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(other_below.len(), 1, "{other_below:?}");
    inset.drop_without_applying_deltas();

    // Dropping the inset (floating window, split screen, keyboard) restores the full layout.
    let back = frame(&ctx, &mut app, Some(0.0));
    assert_eq!(ctx.content_rect().bottom(), 800.0);
    back.drop_without_applying_deltas();
}

fn indicator(height: f32, window_height: f32, surface_top: f32, layout_height: f32) -> Indicator {
    Indicator {
        height,
        window_height,
        surface_top,
        layout_height,
    }
}

#[test]
fn overlap_is_the_part_of_the_indicator_the_surface_covers() {
    // 2000 px tall window, 60 px indicator, surface from the top of the page.
    // Extended under the indicator: surface fills the window.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(2000.0), 60.0);
    // Not extended: the surface stops where the indicator starts.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(1940.0), 0.0);
    // Partly covered, for instance while the layout settles.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(1970.0), 30.0);
    // A page header above the surface shifts it down but the bottom edge decides.
    assert_eq!(indicator(60.0, 2000.0, 100.0, 0.0).overlap(1840.0), 0.0);
    assert_eq!(indicator(60.0, 2000.0, 100.0, 0.0).overlap(1960.0), 60.0);
    // Keyboard RESIZE shrinks the surface well above the indicator.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(1200.0), 0.0);
    // The surface cannot cover more than the indicator itself.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(2400.0), 60.0);
}

#[test]
fn overlap_prefers_the_taller_of_surface_and_layout_height() {
    // Layout already grew under the indicator while the NativeWindow is still short.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 2000.0).overlap(1940.0), 60.0);
    // Surface already grew; layout height not yet reported.
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(2000.0), 60.0);
    // Both short: still no inset (expandSafeArea-only failure mode).
    assert_eq!(indicator(60.0, 2000.0, 0.0, 1940.0).overlap(1940.0), 0.0);
}

#[test]
fn overlap_is_zero_without_indicator_or_with_bad_geometry() {
    // Floating or split-screen windows report no indicator.
    assert_eq!(Indicator::default().overlap(2000.0), 0.0);
    assert_eq!(indicator(0.0, 2000.0, 0.0, 0.0).overlap(2000.0), 0.0);
    // Window height unknown (query failed): never invent an inset.
    assert_eq!(indicator(60.0, 0.0, 0.0, 0.0).overlap(2000.0), 0.0);
    assert_eq!(indicator(60.0, 2000.0, 0.0, 0.0).overlap(0.0), 0.0);
    assert_eq!(indicator(f32::NAN, 2000.0, 0.0, 0.0).overlap(2000.0), 0.0);
    assert_eq!(
        indicator(60.0, 2000.0, f32::INFINITY, 0.0).overlap(2000.0),
        0.0
    );
    assert_eq!(indicator(-60.0, 2000.0, 0.0, 0.0).overlap(2000.0), 0.0);
}
