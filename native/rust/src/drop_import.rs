//! Forward authorized in-memory drops to the editor's existing open/place routing.
use photocraft_ui_egui::{PhotocraftApp, file_open::DropTarget};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug)]
struct DroppedImage {
    name: PathBuf,
    data: Vec<u8>,
}
impl egui::DroppedFile for DroppedImage {
    fn path(&self) -> &std::path::Path {
        &self.name
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        Ok(self.data.clone())
    }
}

fn point(ctx: &egui::Context, x: f32, y: f32) -> Result<egui::Pos2, String> {
    let scale = ctx.pixels_per_point();
    if !x.is_finite() || !y.is_finite() || !scale.is_finite() || scale <= 0. {
        return Err("Invalid image drop position".into());
    }
    let point = egui::pos2(x / scale, y / scale);
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err("Image drop position exceeds coordinate range".into());
    }
    Ok(point)
}

/// Feedback and the final action use the same upstream hit test, including modal windows.
pub fn feedback(
    app: &PhotocraftApp,
    ctx: &egui::Context,
    x: f32,
    y: f32,
    sequence: i32,
) -> Result<String, String> {
    let at = point(ctx, x, y)?;
    let place = app.drop_target(ctx, Some(at)) == DropTarget::Canvas;
    let rect = app.last_canvas_rect;
    let scale = ctx.pixels_per_point();
    Ok(serde_json::json!({
        "sequence": sequence,
        "mode": if place { "place" } else { "open" },
        "left": rect.left() * scale,
        "top": rect.top() * scale,
        "width": rect.width() * scale,
        "height": rect.height() * scale,
    })
    .to_string())
}

pub fn receive(
    app: &mut PhotocraftApp,
    ctx: &egui::Context,
    name: String,
    data: Vec<u8>,
    x: f32,
    y: f32,
) -> Result<(), String> {
    let at = point(ctx, x, y)?;
    if data.is_empty() {
        return Err("Dropped image is empty".into());
    }
    // Never expose an absolute or temporary path: the editor must read the authorized bytes.
    let name = std::path::Path::new(&name)
        .file_name()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dropped-image"));
    let file: egui::DroppedFileHandle = Arc::new(DroppedImage { name, data });
    app.open_dropped(ctx, vec![file], Some(at));
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/drop_import.rs"]
mod tests;
