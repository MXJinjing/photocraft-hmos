//! The surface uses a standard output profile; HarmonyOS owns panel calibration.
//! Canvas pixels arrive in that profile. Other egui pixels arrive in sRGB.
use egui::{Color32, Context, TextureId};
use photocraft_cms::{Builtin, Intent, Transform};
use photocraft_engine::{color_cmds::ColorState, display_color::Display};
use std::{collections::HashMap, sync::Arc};

pub struct DisplayColor {
    pub p3: bool,
    ui: Option<Transform>,
    colors: HashMap<Color32, Color32>,
}

impl DisplayColor {
    pub fn new(p3: bool) -> Result<Self, String> {
        let ui = if p3 {
            Some(
                Transform::new(
                    Builtin::Srgb.profile(),
                    Builtin::DisplayP3.profile(),
                    Intent::RelativeColorimetric,
                    true,
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        Ok(Self {
            p3,
            ui,
            colors: HashMap::new(),
        })
    }

    pub fn install(&self, state: &mut ColorState) {
        let profile = if self.p3 {
            Builtin::DisplayP3
        } else {
            Builtin::Srgb
        };
        // This identifies the compositor's input space, not the physical panel profile.
        state.set_displays(Ok(vec![Display {
            id: 1,
            name: "HarmonyOS system-managed output".into(),
            frame: [0.0; 4],
            profile_name: Some(format!("{} (HarmonyOS output)", profile.description())),
            icc: Some(Arc::new(profile.profile().to_bytes().to_vec())),
        }]));
        state.main_display = Some(1);
    }

    pub fn ui_color(&mut self, color: Color32) -> Color32 {
        let Some(transform) = &self.ui else {
            return color;
        };
        if let Some(mapped) = self.colors.get(&color) {
            return *mapped;
        }
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        // Neutral UI and the font atlas need no conversion. Preserve their exact alpha
        // representation, including egui's additive colors (alpha zero).
        let mapped = if r == g && g == b {
            color
        } else {
            let src = [r, g, b];
            let mut dst = [0; 3];
            transform.convert_u8(&src, 3, &mut dst, 3, false);
            if a == 0 {
                Color32::from_rgba_premultiplied(dst[0], dst[1], dst[2], 0)
            } else {
                Color32::from_rgba_unmultiplied(dst[0], dst[1], dst[2], a)
            }
        };
        if self.colors.len() >= 65536 {
            self.colors.clear();
        }
        self.colors.insert(color, mapped);
        mapped
    }

    pub fn texture(&mut self, ctx: &Context, id: TextureId, image: &mut egui::ImageData) {
        if !self.p3 {
            return;
        }
        let managed = ctx.tex_manager().read().meta(id).is_some_and(|m| {
            m.name
                .starts_with(photocraft_ui_egui::canvas::DISPLAY_TEXTURE_PREFIX)
        });
        if managed {
            return;
        }
        let egui::ImageData::Color(image) = image;
        if image
            .pixels
            .iter()
            .all(|p| p.r() == p.g() && p.g() == p.b())
        {
            return;
        }
        for pixel in &mut Arc::make_mut(image).pixels {
            *pixel = self.ui_color(*pixel);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/display_color.rs"]
#[allow(clippy::unwrap_used)]
mod tests;
