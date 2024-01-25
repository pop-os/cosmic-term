use cosmic::iced::keyboard::{KeyCode, Modifiers};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

use crate::Action;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Modifier {
    Super,
    Ctrl,
    Alt,
    Shift,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct KeyBind {
    pub modifiers: Vec<Modifier>,
    pub key_code: KeyCode,
}

impl KeyBind {
    pub fn matches(&self, modifiers: Modifiers, key_code: KeyCode) -> bool {
        self.key_code == key_code
            && modifiers.logo() == self.modifiers.contains(&Modifier::Super)
            && modifiers.control() == self.modifiers.contains(&Modifier::Ctrl)
            && modifiers.alt() == self.modifiers.contains(&Modifier::Alt)
            && modifiers.shift() == self.modifiers.contains(&Modifier::Shift)
    }
}

impl fmt::Display for KeyBind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for modifier in self.modifiers.iter() {
            write!(f, "{:?} + ", modifier)?;
        }
        write!(f, "{:?}", self.key_code)
    }
}

//TODO: load from config
pub fn key_binds() -> HashMap<KeyBind, Action> {
    let mut key_binds = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),+ $(,)?], $key_code:ident, $action:ident) => {{
            key_binds.insert(
                KeyBind {
                    modifiers: vec![$(Modifier::$modifier),+],
                    key_code: KeyCode::$key_code,
                },
                Action::$action,
            );
        }};
    }

    // Standard key bindings
    bind!([Ctrl, Shift], A, SelectAll);
    bind!([Ctrl, Shift], C, Copy);
    bind!([Ctrl, Shift], F, Find);
    bind!([Ctrl, Shift], N, WindowNew);
    bind!([Ctrl, Shift], Q, WindowClose);
    bind!([Ctrl, Shift], T, TabNew);
    bind!([Ctrl, Shift], V, Paste);
    bind!([Ctrl, Shift], W, TabClose);

    // Ctrl+Alt+D splits horizontally, Ctrl+Alt+R splits vertically, Ctrl+Shift+X maximizes split
    //TODO: Adjust bindings as desired by UX
    bind!([Ctrl, Alt], D, PaneSplitHorizontal);
    bind!([Ctrl, Alt], R, PaneSplitVertical);
    bind!([Ctrl, Alt], P, PasswordManager);
    bind!([Ctrl, Shift], X, PaneToggleMaximized);

    // Ctrl+Tab and Ctrl+Shift+Tab cycle through tabs
    // Ctrl+Tab is not a special key for terminals and is free to use
    bind!([Ctrl], Tab, TabNext);
    bind!([Ctrl, Shift], Tab, TabPrev);

    // Ctrl+Shift+# activates tabs by index
    bind!([Ctrl, Shift], Key1, TabActivate0);
    bind!([Ctrl, Shift], Key2, TabActivate1);
    bind!([Ctrl, Shift], Key3, TabActivate2);
    bind!([Ctrl, Shift], Key4, TabActivate3);
    bind!([Ctrl, Shift], Key5, TabActivate4);
    bind!([Ctrl, Shift], Key6, TabActivate5);
    bind!([Ctrl, Shift], Key7, TabActivate6);
    bind!([Ctrl, Shift], Key8, TabActivate7);
    bind!([Ctrl, Shift], Key9, TabActivate8);

    // Ctrl+0, Ctrl+-, and Ctrl+= are not special keys for terminals and are free to use
    bind!([Ctrl], Key0, ZoomReset);
    bind!([Ctrl], Minus, ZoomOut);
    bind!([Ctrl], Equals, ZoomIn);

    // Ctrl+Arrows and Ctrl+HJKL move between splits
    bind!([Ctrl, Shift], Left, PaneFocusLeft);
    bind!([Ctrl, Shift], H, PaneFocusLeft);
    bind!([Ctrl, Shift], Down, PaneFocusDown);
    bind!([Ctrl, Shift], J, PaneFocusDown);
    bind!([Ctrl, Shift], Up, PaneFocusUp);
    bind!([Ctrl, Shift], K, PaneFocusUp);
    bind!([Ctrl, Shift], Right, PaneFocusRight);
    bind!([Ctrl, Shift], L, PaneFocusRight);

    key_binds
}
