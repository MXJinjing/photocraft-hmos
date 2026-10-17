use super::*;

#[test]
fn to_points_converts_pixels_and_rejects_bad_values() {
    assert_eq!(to_points(60.0, 2.0, 800.0), 30.0);
    assert_eq!(to_points(-5.0, 2.0, 800.0), 0.0);
    assert_eq!(to_points(60.0, 0.0, 800.0), 0.0);
    assert_eq!(to_points(f32::NAN, 2.0, 800.0), 0.0);
    assert_eq!(to_points(60.0, f32::INFINITY, 800.0), 0.0);
    // UI zoom enters through the scale, so the same pixels become fewer points when zoomed in.
    assert_eq!(to_points(60.0, 3.0, 800.0), 20.0);
}

#[test]
fn to_points_never_takes_more_than_a_quarter_of_the_window() {
    assert_eq!(to_points(10_000.0, 1.0, 800.0), 200.0);
    assert_eq!(to_points(10_000.0, 1.0, 0.0), 0.0);
}

fn frame(
    ctx: &egui::Context,
    app: &mut PhotocraftApp,
    insets_points: Option<(f32, f32)>,
) -> egui::FullOutput {
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280., 800.),
        )),
        safe_area_insets: insets_points.map(|(top, bottom)| insets(top, bottom)),
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

    let plain = frame(&ctx, &mut app, Some((0.0, 0.0)));
    assert_eq!(ctx.content_rect().bottom(), 800.0);
    let bottoms: Vec<f32> = fills(&plain, chrome).iter().map(|r| r.bottom()).collect();
    assert!(
        bottoms.contains(&800.0),
        "status bar reaches the edge: {bottoms:?}"
    );
    plain.drop_without_applying_deltas();

    let inset = frame(&ctx, &mut app, Some((0.0, 24.0)));
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
    let back = frame(&ctx, &mut app, Some((0.0, 0.0)));
    assert_eq!(ctx.content_rect().bottom(), 800.0);
    back.drop_without_applying_deltas();
}

#[test]
fn status_bar_inset_pushes_the_title_bar_down_and_fills_the_top_strip() {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut app = app();
    for _ in 0..3 {
        frame(&ctx, &mut app, None).drop_without_applying_deltas();
    }
    let chrome = photocraft_ui_egui::theme::Tokens::get(&ctx).chrome;
    let output = frame(&ctx, &mut app, Some((30.0, 24.0)));
    assert_eq!(ctx.content_rect().top(), 30.0);
    assert_eq!(ctx.content_rect().bottom(), 776.0);
    let rects = fills(&output, chrome);
    assert!(
        rects
            .iter()
            .any(|r| r.top() == 0.0 && r.bottom() == 30.0 && r.width() == 1280.0),
        "top strip: {rects:?}"
    );
    assert!(
        rects
            .iter()
            .any(|r| r.top() == 776.0 && r.bottom() == 800.0 && r.width() == 1280.0),
        "bottom strip: {rects:?}"
    );
    output.drop_without_applying_deltas();
}

#[test]
fn strips_are_opaque_even_when_the_theme_colour_is_translucent() {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let app = app();
    for bottom in [false, true] {
        let color = strip_color(&app, &ctx, bottom);
        assert_eq!(color.a(), 255, "bottom={bottom}");
    }
}

#[test]
fn zero_insets_use_the_plain_layout_without_extra_shapes() {
    let ctx = egui::Context::default();
    PhotocraftApp::setup_context(&ctx, Default::default());
    let mut app = app();
    for _ in 0..3 {
        frame(&ctx, &mut app, None).drop_without_applying_deltas();
    }
    let output = frame(&ctx, &mut app, Some((0.0, 0.0)));
    assert_eq!(ctx.content_rect(), ctx.viewport_rect());
    output.drop_without_applying_deltas();
}
