use cosmic::widget::menu::key_bind::{KeyBind, Modifier};
use cosmic::{iced::keyboard::Key, iced_core::keyboard::key::Named};
use std::collections::HashMap;

use crate::Action;

//TODO: load from config
pub fn key_binds() -> HashMap<KeyBind, Action> {
    let mut key_binds = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),* $(,)?], $key:expr, $action:ident) => {{
            key_binds.insert(
                KeyBind {
                    modifiers: vec![$(Modifier::$modifier),*],
                    key: $key,
                },
                Action::$action,
            );
        }};
    }

    // Standard key bindings
    bind!([Ctrl, Shift], Key::Character("A".into()), SelectAll);
    bind!([Ctrl, Shift], Key::Character("C".into()), Copy);
    bind!([], Key::Named(Named::Copy), Copy);
    bind!([Ctrl], Key::Character("c".into()), CopyOrSigint);
    bind!([Ctrl, Shift], Key::Character("F".into()), Find);
    bind!([Ctrl, Shift], Key::Character("N".into()), WindowNew);
    bind!([Ctrl, Shift], Key::Character("Q".into()), WindowClose);
    bind!([Ctrl, Shift], Key::Character("T".into()), TabNew);
    bind!([Ctrl, Shift], Key::Character("V".into()), Paste);
    bind!([], Key::Named(Named::Paste), Paste);
    bind!([Shift], Key::Named(Named::Insert), PastePrimary);
    bind!([Ctrl, Shift], Key::Character("W".into()), TabClose);
    bind!([Ctrl], Key::Character(",".into()), Settings);
    bind!([], Key::Named(Named::F11), ToggleFullscreen);

    // Ctrl+Alt+D splits horizontally, Ctrl+Alt+R splits vertically, Ctrl+Shift+X maximizes split
    //TODO: Adjust bindings as desired by UX
    bind!([Ctrl, Alt], Key::Character("d".into()), PaneSplitHorizontal);
    bind!([Ctrl, Alt], Key::Character("r".into()), PaneSplitVertical);
    bind!(
        [Ctrl, Shift],
        Key::Character("X".into()),
        PaneToggleMaximized
    );

    // Ctrl+Tab and Ctrl+Shift+Tab cycle through tabs
    // Ctrl+Tab is not a special key for terminals and is free to use
    bind!([Ctrl], Key::Named(Named::Tab), TabNext);
    bind!([Ctrl, Shift], Key::Named(Named::Tab), TabPrev);

    // Ctrl+Shift+# activates tabs by index
    bind!([Ctrl, Shift], Key::Character("1".into()), TabActivate0);
    bind!([Ctrl, Shift], Key::Character("2".into()), TabActivate1);
    bind!([Ctrl, Shift], Key::Character("3".into()), TabActivate2);
    bind!([Ctrl, Shift], Key::Character("4".into()), TabActivate3);
    bind!([Ctrl, Shift], Key::Character("5".into()), TabActivate4);
    bind!([Ctrl, Shift], Key::Character("6".into()), TabActivate5);
    bind!([Ctrl, Shift], Key::Character("7".into()), TabActivate6);
    bind!([Ctrl, Shift], Key::Character("8".into()), TabActivate7);
    bind!([Ctrl, Shift], Key::Character("9".into()), TabActivate8);

    // Ctrl+0, Ctrl+-, and Ctrl+= are not special keys for terminals and are free to use
    bind!([Ctrl], Key::Character("0".into()), ZoomReset);
    bind!([Ctrl], Key::Character("-".into()), ZoomOut);
    bind!([Ctrl], Key::Character("=".into()), ZoomIn);
    bind!([Ctrl], Key::Character("+".into()), ZoomIn);

    // Ctrl+Arrows and Ctrl+HJKL move between splits
    bind!([Ctrl, Shift], Key::Named(Named::ArrowLeft), PaneFocusLeft);
    bind!([Ctrl, Shift], Key::Character("H".into()), PaneFocusLeft);
    bind!([Ctrl, Shift], Key::Named(Named::ArrowDown), PaneFocusDown);
    bind!([Ctrl, Shift], Key::Character("J".into()), PaneFocusDown);
    bind!([Ctrl, Shift], Key::Named(Named::ArrowUp), PaneFocusUp);
    bind!([Ctrl, Shift], Key::Character("K".into()), PaneFocusUp);
    bind!([Ctrl, Shift], Key::Named(Named::ArrowRight), PaneFocusRight);
    bind!([Ctrl, Shift], Key::Character("L".into()), PaneFocusRight);

    // CTRL+Alt+L clears the scrollback.
    bind!([Ctrl, Alt], Key::Character("L".into()), ClearScrollback);

    key_binds
}
