use super::{PhotocraftApp, RichText, Tokens, label, vec2, widgets};

// ----------------------------------------------------------------------------- status bar

/// The status bar's height and the height of the controls in it (zoom field, Fit), which leaves
/// them an even margin above and below inside the bar (#2823).
pub fn status_bar_heights(t: &Tokens) -> (f32, f32) {
    if t.pro { (24.0, 18.0) } else { (30.0, 22.0) }
}

pub fn status_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let button_size = crate::status_panel_controls::button_size(t.pro);
    egui::Panel::bottom("status_bar")
        .exact_size(button_size + 2.0)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(10, 0)))
        .show(ui, |ui| {
            let control_h = status_bar_heights(&t).1;
            // Draw the divider inside the bar so the original 30-point Fit button fits without
            // increasing the bar by the panel separator's extra point.
            let rect = ui.max_rect();
            ui.painter().hline(rect.x_range(), rect.top(), ui.visuals().widgets.noninteractive.bg_stroke);
            let body_rect = crate::status_panel_controls::show(app, ui, button_size);
            let mut body = ui.new_child(egui::UiBuilder::new().id_salt("status-body").max_rect(body_rect));
            body.set_clip_rect(body.clip_rect().intersect(body_rect));
            // Read from the right in a right-to-left UI, with the trailing group at the left.
            crate::direction::horizontal_centered(&mut body, |ui| {
                if t.pro {
                    crate::chrome_ui::status_bar_pro(app, ui);
                    // Background job progress with Cancel, at the far end (#210).
                    ui.with_layout(crate::direction::trailing(), |ui| {
                        crate::background_redraw::status(app, ui);
                        crate::chrome_ui::render_path_status(app, ui);
                    });
                    return;
                }
                if let (Some(st), Some(i)) = (app.session.active(), app.session.active_index()) {
                    let (w, h, mode, bits, layers) =
                        (st.doc.size.width, st.doc.size.height, crate::canvas::mode_label(&st.doc), st.doc.depth.bits(), st.doc.layer_count());
                    let mut pct = app.ui.views[i].zoom * 100.0;
                    if widgets::compact_wrapping_value_field(ui, &mut pct, crate::zoom_levels::percent_range(&app.ui.views[i]), "%", vec2(78.0, control_h))
                        .accessible_name(tl!("Zoom"))
                        .changed()
                    {
                        app.ui.views[i].zoom = crate::zoom_levels::clamp(pct / 100.0, app.ui.views[i].doc_size);
                        app.ui.views[i].zoom_animating = false;
                        app.ui.views[i].zoom_anchor = None;
                        app.ui.views[i].fit_pending = false;
                    }
                    crate::status_panel_controls::fit_button(app, ui, zoom_rect);
                    widgets::vline(ui, 16.0);
                    label(ui, &crate::i18n::fmt(tl!("{mode} Color · {bits} bit"), &[("mode", mode), ("bits", &bits.to_string())]));
                    widgets::vline(ui, 16.0);
                    label(ui, &format!("{w} × {h} px"));
                    widgets::vline(ui, 16.0);
                    label(ui, &crate::i18n::trn(crate::i18n::current(), layers as u64, "{n} layer", "{n} layers"));
                } else {
                    label(ui, tl!("No document"));
                }
                if !app.ui.status.is_empty() {
                    widgets::vline(ui, 16.0);
                    let is_err = app.ui.status_error || app.ui.status.starts_with("Couldn");
                    ui.label(RichText::new(&app.ui.status).color(if is_err { t.warning } else { t.text_faint }));
                }
                ui.with_layout(crate::direction::trailing(), |ui| {
                    crate::background_redraw::status(app, ui);
                    crate::chrome_ui::render_path_status(app, ui);
                });
            });
        });
}
