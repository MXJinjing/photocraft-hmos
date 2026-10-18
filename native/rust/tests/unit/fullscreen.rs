use super::*;
use photocraft_ui_egui::{PhotocraftApp, window_fullscreen};
use std::{cell::RefCell, rc::Rc};

#[test]
fn state_tokens_parse_and_garbage_is_ignored() {
    assert_eq!(parse_state("on"), Some(true));
    assert_eq!(parse_state(" TRUE "), Some(true));
    assert_eq!(parse_state("off"), Some(false));
    assert_eq!(parse_state("0"), Some(false));
    assert_eq!(parse_state(""), None);
    assert_eq!(parse_state("maybe"), None);
}

#[test]
fn starts_off_and_only_the_host_report_changes_it() {
    let fullscreen = Fullscreen::default();
    assert!(!fullscreen.active());
    assert!(!fullscreen.report("off"), "no change");
    assert!(!fullscreen.report("garbage"));
    assert!(fullscreen.report("on"));
    assert!(fullscreen.active());
    assert!(!fullscreen.report("on"));
    // The system left full screen by itself (floating window, gesture): the mirror follows.
    assert!(fullscreen.report("off"));
    assert!(!fullscreen.active());
}

#[test]
fn requests_ask_for_the_opposite_state_and_do_not_flip_it_early() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let fullscreen = Fullscreen::default();
    let log = Rc::clone(&asked);
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.services.window_fullscreen =
        Some(fullscreen.service(move |kind| log.borrow_mut().push(kind.to_string())));

    assert!(window_fullscreen::toggle(&app));
    assert!(!window_fullscreen::active(&app), "state waits for ArkTS");
    fullscreen.report("on");
    assert!(window_fullscreen::active(&app));
    assert!(!window_fullscreen::toggle(&app));
    assert_eq!(*asked.borrow(), vec![ENTER.to_string(), EXIT.to_string()]);
}

#[test]
fn window_menu_item_sits_right_below_theme_and_follows_the_state() {
    let fullscreen = Fullscreen::default();
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let titles = |app: &PhotocraftApp| {
        photocraft_ui_egui::menus::menu_items(app)
            .into_iter()
            .map(|item| (item.id, item.label, item.path))
            .collect::<Vec<_>>()
    };
    assert!(
        !titles(&app)
            .iter()
            .any(|(id, ..)| id == window_fullscreen::COMMAND),
        "default hosts show no item"
    );
    app.services.window_fullscreen = Some(fullscreen.service(|_| {}));
    let items = titles(&app);
    let at = items
        .iter()
        .position(|(id, ..)| id == window_fullscreen::COMMAND)
        .unwrap();
    assert_eq!(items[at].1, "Enter Full Screen");
    assert_eq!(items[at].2, vec!["Window".to_string()]);
    assert_eq!(
        items[at - 1].0,
        "window.theme.system",
        "directly after the Theme submenu"
    );
    fullscreen.report("on");
    let items = titles(&app);
    let at = items
        .iter()
        .position(|(id, ..)| id == window_fullscreen::COMMAND)
        .unwrap();
    assert_eq!(items[at].1, "Exit Full Screen");
}
