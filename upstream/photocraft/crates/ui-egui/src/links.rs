//! Community and project links: the Help menu, the About dialog, the start screen and the title
//! bar all open these. URLs follow the crafting-app pattern (`getartcraft.com/apps/<app>`,
//! `github.com/storytold/<app>`).

use serde_json::{Value, json};

use crate::PhotocraftApp;

/// Whether this host exposes community and project entry points.
pub const SHOW_PROJECT_LINKS: bool = !cfg!(feature = "harmonyos-ui");

pub const DISCORD: &str = "https://discord.gg/artcraft";
pub const ARTCRAFT_WEBSITE: &str = "https://getartcraft.com";
pub const APP_PAGE: &str = "https://getartcraft.com/apps/photocraft";
#[cfg(not(feature = "harmonyos-ui"))]
pub const GITHUB: &str = "https://github.com/storytold/photocraft";
#[cfg(not(feature = "harmonyos-ui"))]
pub const ISSUES: &str = "https://github.com/storytold/photocraft/issues";
// HarmonyOS-specific problems belong to the fork rather than the upstream project.
#[cfg(feature = "harmonyos-ui")]
pub const GITHUB: &str = "https://github.com/MXJinjing/photocraft-hmos";
#[cfg(feature = "harmonyos-ui")]
pub const ISSUES: &str = "https://github.com/MXJinjing/photocraft-hmos/issues";

/// Help-menu link commands: (id, url). Labels live in `menus::UI_COMMANDS`.
pub const COMMANDS: &[(&str, &str)] =
    &[("help.discord", DISCORD), ("help.website", APP_PAGE), ("help.artcraftWebsite", ARTCRAFT_WEBSITE), ("help.github", GITHUB), ("help.reportIssue", ISSUES)];

pub fn url_for(id: &str) -> Option<&'static str> {
    COMMANDS.iter().find(|c| c.0 == id).map(|c| c.1)
}

/// Open `url` in the system browser (a new tab on the web) and note it in the status bar. Prefers
/// the platform `open_url` service (reliable on Windows/macOS/Linux); falls back to `ctx.open_url`.
pub fn open(app: &mut PhotocraftApp, ctx: &egui::Context, url: &str) -> Value {
    let opened = app.services.open_url.as_ref().map(|f| f(url).is_ok()).unwrap_or(false);
    if !opened {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
    app.ui.status = format!("Opened {url}");
    json!({"url": url})
}

/// The prominent "Join us on Discord" button.
pub fn discord_button(app: &mut PhotocraftApp, ui: &mut egui::Ui, min_width: f32) -> egui::Response {
    let r = crate::widgets::primary_button(ui, tl!("Join us on Discord"), min_width).on_hover_text(DISCORD);
    if r.clicked() {
        open(app, ui.ctx(), DISCORD);
    }
    r
}

/// "PhotoCraft website · GitHub · ArtCraft" as links, centred. Clicks route through [`open`] (the
/// platform browser service) rather than `ui.hyperlink_to`, which uses the unreliable `ctx.open_url`.
pub fn link_row(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let links = [(tl!("PhotoCraft website"), APP_PAGE), (tl!("GitHub"), GITHUB), (tl!("ArtCraft"), ARTCRAFT_WEBSITE)];
    let font = egui::FontId::proportional(12.5);
    let sep = "  ·  ";
    let width: f32 = links.iter().map(|(l, _)| ui.painter().layout_no_wrap((*l).into(), font.clone(), t.text).size().x).sum::<f32>()
        + 2.0 * ui.painter().layout_no_wrap(sep.into(), font.clone(), t.text).size().x;
    let mut clicked: Option<&str> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        for (i, (label, url)) in links.iter().enumerate() {
            if i > 0 {
                ui.label(egui::RichText::new(sep).font(font.clone()).color(t.text_faint));
            }
            if ui.link(egui::RichText::new(*label).font(font.clone()).color(t.accent)).on_hover_text(*url).clicked() {
                clicked = Some(url);
            }
        }
    });
    if let Some(url) = clicked {
        open(app, ui.ctx(), url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_open_through_the_platform_service() {
        // Issue #14: links (Help menu, Discord button, start-page links) must open in the browser.
        // They route through the `open_url` service rather than the unreliable `ctx.open_url`.
        use std::sync::{Arc, Mutex};
        let opened: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let rec = opened.clone();
        let services = crate::Services {
            open_url: Some(Box::new(move |u: &str| {
                rec.lock().unwrap().push(u.to_string());
                Ok(())
            })),
            ..Default::default()
        };
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
        let ctx = egui::Context::default();
        // Every Help-menu link id reaches the service with its URL.
        for (id, url) in COMMANDS {
            crate::menus::invoke(&mut app, &ctx, id, json!({})).unwrap();
            assert_eq!(opened.lock().unwrap().last().map(String::as_str), Some(*url), "{id}");
        }
        // The direct open() helper (Discord button / start-page links) also uses it.
        open(&mut app, &ctx, DISCORD);
        assert_eq!(opened.lock().unwrap().last().map(String::as_str), Some(DISCORD));
    }

    #[cfg(not(feature = "harmonyos-ui"))]
    #[test]
    fn help_menu_lists_links_then_separator_then_system_info_and_about() {
        let app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        let help: Vec<String> = crate::menus::menu_items(&app).into_iter().filter(|i| i.path == ["Help"]).map(|i| i.id).collect();
        assert_eq!(help, ["help.discord", "help.website", "help.artcraftWebsite", "help.github", "help.reportIssue", "---", "help.systemInfo", "help.about"]);
        for (id, _) in COMMANDS {
            assert!(crate::menus::is_live(id) && crate::menus::is_enabled(&app, id), "{id}");
        }
    }

    #[cfg(feature = "harmonyos-ui")]
    #[test]
    fn harmonyos_help_menu_keeps_fork_links_and_local_information() {
        let app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        let help: Vec<String> = crate::menus::menu_items(&app).into_iter().filter(|i| i.path == ["Help"]).map(|i| i.id).collect();
        assert_eq!(help, ["help.github", "help.reportIssue", "---", "help.systemInfo", "help.about"]);
        let links: Vec<&str> = crate::menus::UI_COMMANDS.iter().filter(|c| url_for(c.0).is_some()).map(|c| c.0).collect();
        assert_eq!(links, ["help.github", "help.reportIssue"]);
        assert_eq!(url_for("help.github"), Some("https://github.com/MXJinjing/photocraft-hmos"));
        assert_eq!(url_for("help.reportIssue"), Some("https://github.com/MXJinjing/photocraft-hmos/issues"));
    }

    #[cfg(feature = "harmonyos-ui")]
    #[test]
    fn harmonyos_home_and_about_hide_project_links_and_credit_tabs() {
        use egui_kittest::{Harness, kittest::Queryable};
        let app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
            |ui, app| {
                app.host_logic(ui.ctx());
                app.host_ui(ui);
            },
            app,
        );
        PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
        h.run_steps(3);
        for label in ["Discord", "Join us on Discord", "PhotoCraft website", "GitHub", "ArtCraft"] {
            assert!(h.query_by_label(label).is_none(), "home: {label}");
        }
        for tab in ["about", "contributors", "models"] {
            h.state_mut().ui.dialogs.clear();
            h.state_mut().ui.open_dialog(crate::state::DialogKind::About, serde_json::from_value(json!({"tab": tab})).unwrap());
            h.run_steps(3);
            assert!(h.query_by_label("About PhotoCraft").is_some());
            assert!(h.query_by_label_contains("Version ").is_some());
            for label in ["Contributors", "Models", "Join us on Discord", "PhotoCraft website", "GitHub", "ArtCraft"] {
                assert!(h.query_by_label(label).is_none(), "about {tab}: {label}");
            }
        }
    }

    #[test]
    fn link_commands_open_their_urls() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        for (id, url) in [
            ("help.discord", "https://discord.gg/artcraft"),
            ("help.website", "https://getartcraft.com/apps/photocraft"),
            ("help.github", GITHUB),
            ("help.reportIssue", ISSUES),
        ] {
            let r = crate::menus::invoke(&mut app, &ctx, id, serde_json::json!({})).unwrap();
            assert_eq!(r["url"], url);
            assert_eq!(app.ui.status, format!("Opened {url}"));
        }
    }
}
