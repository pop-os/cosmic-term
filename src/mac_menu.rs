#[cfg(target_os = "macos")]
use {
    crate::{Action, Config, fl},
    muda::{Menu, MenuItem, PredefinedMenuItem, Submenu},
};

#[cfg(target_os = "macos")]
pub fn init_mac_menu() -> Menu {
    let menu_bar = Menu::new();

    // App Menu (macOS default first menu)
    let app_menu = Submenu::new("Terminal COSMIC-MAC", true);
    app_menu.append_items(&[
        &MenuItem::with_id(
            "About",
            fl!("menu-about"),
            true,
            None,
        ),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            "Settings",
            fl!("menu-settings"),
            true,
            Some("Cmd+,".parse().ok()).flatten(),
        ),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(None),
    ]).unwrap();

    let file_menu = Submenu::new(fl!("file"), true);
    file_menu.append_items(&[
        &MenuItem::with_id(
            "TabNew",
            fl!("new-tab"),
            true,
            Some("Cmd+T".parse().ok()).flatten(),
        ),
        &MenuItem::with_id(
            "WindowNew",
            fl!("new-window"),
            true,
            Some("Cmd+N".parse().ok()).flatten(),
        ),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            "TabClose",
            fl!("close-tab"),
            true,
            Some("Cmd+W".parse().ok()).flatten(),
        ),
    ]).unwrap();

    let edit_menu = Submenu::new(fl!("edit"), true);
    edit_menu.append_items(&[
        &PredefinedMenuItem::copy(Some(fl!("copy").as_str())),
        &PredefinedMenuItem::paste(Some(fl!("paste").as_str())),
        &PredefinedMenuItem::select_all(Some(fl!("select-all").as_str())),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            "ClearScrollback",
            fl!("clear-scrollback"),
            true,
            Some("Cmd+K".parse().ok()).flatten(),
        ),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            "Find",
            fl!("find"),
            true,
            Some("Cmd+F".parse().ok()).flatten(),
        ),
    ]).unwrap();

    let view_menu = Submenu::new(fl!("view"), true);
    view_menu.append_items(&[
        &MenuItem::with_id(
            "ZoomIn",
            fl!("zoom-in"),
            true,
            Some("Cmd+Equal".parse().ok()).flatten(),
        ),
        &MenuItem::with_id(
            "ZoomOut",
            fl!("zoom-out"),
            true,
            Some("Cmd+Minus".parse().ok()).flatten(),
        ),
        &MenuItem::with_id(
            "ZoomReset",
            fl!("zoom-reset"),
            true,
            Some("Cmd+0".parse().ok()).flatten(),
        ),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            "TabNext",
            fl!("next-tab"),
            true,
            Some("Cmd+Tab".parse().ok()).flatten(),
        ),
        &MenuItem::with_id(
            "TabPrev",
            fl!("previous-tab"),
            true,
            Some("Cmd+Shift+Tab".parse().ok()).flatten(),
        ),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            "PaneSplitHorizontal",
            fl!("split-horizontal"),
            true,
            None,
        ),
        &MenuItem::with_id(
            "PaneSplitVertical",
            fl!("split-vertical"),
            true,
            None,
        ),
        &MenuItem::with_id(
            "PaneToggleMaximized",
            fl!("pane-toggle-maximize"),
            true,
            None,
        ),
    ]).unwrap();

    menu_bar.append_items(&[&app_menu, &file_menu, &edit_menu, &view_menu]).unwrap();
    menu_bar.init_for_nsapp();
    menu_bar
}
