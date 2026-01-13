// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget::menu::key_bind::{KeyBind, Modifier};
use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    iced::keyboard::{Key, Modifiers},
    iced_core::keyboard::key::Named,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::Action;

pub const SHORTCUTS_CONFIG_ID: &str = "com.system76.CosmicTerm.Shortcuts";
pub const SHORTCUTS_CONFIG_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ModifierName {
    Ctrl,
    Shift,
    Alt,
    Super,
}

impl ModifierName {
    fn to_modifier(self) -> Modifier {
        match self {
            Self::Ctrl => Modifier::Ctrl,
            Self::Shift => Modifier::Shift,
            Self::Alt => Modifier::Alt,
            Self::Super => Modifier::Super,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Binding {
    pub modifiers: Vec<ModifierName>,
    pub key: String,
}

impl Binding {
    fn to_key_bind(&self) -> Option<KeyBind> {
        let key = key_from_string(&self.key)?;
        let mut modifiers = Vec::new();
        for modifier in [
            ModifierName::Ctrl,
            ModifierName::Shift,
            ModifierName::Alt,
            ModifierName::Super,
        ] {
            if self.modifiers.contains(&modifier) {
                modifiers.push(modifier.to_modifier());
            }
        }

        Some(KeyBind { modifiers, key })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeyBindAction {
    Unbind,
    ClearScrollback,
    Copy,
    CopyOrSigint,
    Find,
    PaneFocusDown,
    PaneFocusLeft,
    PaneFocusRight,
    PaneFocusUp,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneToggleMaximized,
    Paste,
    PastePrimary,
    #[cfg_attr(not(feature = "password_manager"), allow(dead_code))]
    PasswordManager,
    SelectAll,
    Settings,
    TabActivate0,
    TabActivate1,
    TabActivate2,
    TabActivate3,
    TabActivate4,
    TabActivate5,
    TabActivate6,
    TabActivate7,
    TabActivate8,
    TabClose,
    TabNew,
    TabNext,
    TabPrev,
    ToggleFullscreen,
    WindowClose,
    WindowNew,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl KeyBindAction {
    fn to_action(self) -> Option<Action> {
        match self {
            Self::Unbind => None,
            Self::ClearScrollback => Some(Action::ClearScrollback),
            Self::Copy => Some(Action::Copy),
            Self::CopyOrSigint => Some(Action::CopyOrSigint),
            Self::Find => Some(Action::Find),
            Self::PaneFocusDown => Some(Action::PaneFocusDown),
            Self::PaneFocusLeft => Some(Action::PaneFocusLeft),
            Self::PaneFocusRight => Some(Action::PaneFocusRight),
            Self::PaneFocusUp => Some(Action::PaneFocusUp),
            Self::PaneSplitHorizontal => Some(Action::PaneSplitHorizontal),
            Self::PaneSplitVertical => Some(Action::PaneSplitVertical),
            Self::PaneToggleMaximized => Some(Action::PaneToggleMaximized),
            Self::Paste => Some(Action::Paste),
            Self::PastePrimary => Some(Action::PastePrimary),
            Self::SelectAll => Some(Action::SelectAll),
            Self::Settings => Some(Action::Settings),
            Self::TabActivate0 => Some(Action::TabActivate0),
            Self::TabActivate1 => Some(Action::TabActivate1),
            Self::TabActivate2 => Some(Action::TabActivate2),
            Self::TabActivate3 => Some(Action::TabActivate3),
            Self::TabActivate4 => Some(Action::TabActivate4),
            Self::TabActivate5 => Some(Action::TabActivate5),
            Self::TabActivate6 => Some(Action::TabActivate6),
            Self::TabActivate7 => Some(Action::TabActivate7),
            Self::TabActivate8 => Some(Action::TabActivate8),
            Self::TabClose => Some(Action::TabClose),
            Self::TabNew => Some(Action::TabNew),
            Self::TabNext => Some(Action::TabNext),
            Self::TabPrev => Some(Action::TabPrev),
            Self::ToggleFullscreen => Some(Action::ToggleFullscreen),
            Self::WindowClose => Some(Action::WindowClose),
            Self::WindowNew => Some(Action::WindowNew),
            Self::ZoomIn => Some(Action::ZoomIn),
            Self::ZoomOut => Some(Action::ZoomOut),
            Self::ZoomReset => Some(Action::ZoomReset),
            Self::PasswordManager => {
                #[cfg(feature = "password_manager")]
                {
                    Some(Action::PasswordManager)
                }
                #[cfg(not(feature = "password_manager"))]
                {
                    None
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Shortcuts(pub BTreeMap<Binding, KeyBindAction>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingSource {
    Default,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinding {
    pub binding: Binding,
    pub source: BindingSource,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, CosmicConfigEntry)]
pub struct ShortcutsConfig {
    pub defaults: Shortcuts,
    pub custom: Shortcuts,
}

impl ShortcutsConfig {
    pub fn key_binds(&self) -> HashMap<KeyBind, Action> {
        let mut binds = HashMap::new();
        let defaults = self.defaults_or_fallback();
        insert_shortcuts(&defaults, &mut binds, false);
        insert_shortcuts(&self.custom, &mut binds, true);
        binds
    }

    pub fn bindings_for_action(&self, action: KeyBindAction) -> Vec<ResolvedBinding> {
        let mut bindings = Vec::new();
        let defaults = self.defaults_or_fallback();

        for (binding, default_action) in &defaults.0 {
            if *default_action != action {
                continue;
            }

            match self.custom.0.get(binding) {
                Some(KeyBindAction::Unbind) => (),
                Some(custom_action) => {
                    if *custom_action == action {
                        bindings.push(ResolvedBinding {
                            binding: binding.clone(),
                            source: BindingSource::Custom,
                        });
                    }
                }
                None => bindings.push(ResolvedBinding {
                    binding: binding.clone(),
                    source: BindingSource::Default,
                }),
            }
        }

        for (binding, custom_action) in &self.custom.0 {
            if *custom_action == action
                && !bindings.iter().any(|resolved| resolved.binding == *binding)
            {
                bindings.push(ResolvedBinding {
                    binding: binding.clone(),
                    source: BindingSource::Custom,
                });
            }
        }

        bindings
    }

    fn defaults_or_fallback(&self) -> Shortcuts {
        if self.defaults.0.is_empty() {
            fallback_shortcuts()
        } else {
            self.defaults.clone()
        }
    }
}

pub fn load() -> (Option<cosmic_config::Config>, ShortcutsConfig) {
    match cosmic_config::Config::new(SHORTCUTS_CONFIG_ID, SHORTCUTS_CONFIG_VERSION) {
        Ok(config_handler) => {
            let config = match ShortcutsConfig::get_entry(&config_handler) {
                Ok(config) => config,
                Err((errors, config)) => {
                    log::info!("errors loading shortcuts config: {:?}", errors);
                    config
                }
            };
            (Some(config_handler), config)
        }
        Err(err) => {
            log::error!("failed to create shortcuts config handler: {}", err);
            (None, ShortcutsConfig::default())
        }
    }
}

pub fn action_label(action: KeyBindAction) -> &'static str {
    match action {
        KeyBindAction::Unbind => "Unbind",
        KeyBindAction::ClearScrollback => "Clear scrollback",
        KeyBindAction::Copy => "Copy",
        KeyBindAction::CopyOrSigint => "Copy or SIGINT",
        KeyBindAction::Find => "Find",
        KeyBindAction::PaneFocusDown => "Focus pane down",
        KeyBindAction::PaneFocusLeft => "Focus pane left",
        KeyBindAction::PaneFocusRight => "Focus pane right",
        KeyBindAction::PaneFocusUp => "Focus pane up",
        KeyBindAction::PaneSplitHorizontal => "Split pane horizontally",
        KeyBindAction::PaneSplitVertical => "Split pane vertically",
        KeyBindAction::PaneToggleMaximized => "Toggle pane maximized",
        KeyBindAction::Paste => "Paste",
        KeyBindAction::PastePrimary => "Paste primary",
        KeyBindAction::PasswordManager => "Password manager",
        KeyBindAction::SelectAll => "Select all",
        KeyBindAction::Settings => "Settings",
        KeyBindAction::TabActivate0 => "Activate tab 1",
        KeyBindAction::TabActivate1 => "Activate tab 2",
        KeyBindAction::TabActivate2 => "Activate tab 3",
        KeyBindAction::TabActivate3 => "Activate tab 4",
        KeyBindAction::TabActivate4 => "Activate tab 5",
        KeyBindAction::TabActivate5 => "Activate tab 6",
        KeyBindAction::TabActivate6 => "Activate tab 7",
        KeyBindAction::TabActivate7 => "Activate tab 8",
        KeyBindAction::TabActivate8 => "Activate tab 9",
        KeyBindAction::TabClose => "Close tab",
        KeyBindAction::TabNew => "New tab",
        KeyBindAction::TabNext => "Next tab",
        KeyBindAction::TabPrev => "Previous tab",
        KeyBindAction::ToggleFullscreen => "Toggle fullscreen",
        KeyBindAction::WindowClose => "Close window",
        KeyBindAction::WindowNew => "New window",
        KeyBindAction::ZoomIn => "Zoom in",
        KeyBindAction::ZoomOut => "Zoom out",
        KeyBindAction::ZoomReset => "Reset zoom",
    }
}

pub struct ShortcutGroup {
    pub title: &'static str,
    pub actions: Vec<KeyBindAction>,
}

pub fn shortcut_groups() -> Vec<ShortcutGroup> {
    let mut groups = Vec::new();
    groups.push(ShortcutGroup {
        title: "Clipboard",
        actions: vec![
            KeyBindAction::SelectAll,
            KeyBindAction::Copy,
            KeyBindAction::CopyOrSigint,
            KeyBindAction::Paste,
            KeyBindAction::PastePrimary,
            KeyBindAction::Find,
        ],
    });
    groups.push(ShortcutGroup {
        title: "Tabs",
        actions: vec![
            KeyBindAction::TabNew,
            KeyBindAction::TabClose,
            KeyBindAction::TabNext,
            KeyBindAction::TabPrev,
            KeyBindAction::TabActivate0,
            KeyBindAction::TabActivate1,
            KeyBindAction::TabActivate2,
            KeyBindAction::TabActivate3,
            KeyBindAction::TabActivate4,
            KeyBindAction::TabActivate5,
            KeyBindAction::TabActivate6,
            KeyBindAction::TabActivate7,
            KeyBindAction::TabActivate8,
        ],
    });
    groups.push(ShortcutGroup {
        title: "Splits",
        actions: vec![
            KeyBindAction::PaneSplitHorizontal,
            KeyBindAction::PaneSplitVertical,
            KeyBindAction::PaneToggleMaximized,
            KeyBindAction::PaneFocusLeft,
            KeyBindAction::PaneFocusRight,
            KeyBindAction::PaneFocusUp,
            KeyBindAction::PaneFocusDown,
        ],
    });
    groups.push(ShortcutGroup {
        title: "Window",
        actions: vec![
            KeyBindAction::WindowNew,
            KeyBindAction::WindowClose,
            KeyBindAction::ToggleFullscreen,
            KeyBindAction::Settings,
        ],
    });
    groups.push(ShortcutGroup {
        title: "Zoom",
        actions: vec![
            KeyBindAction::ZoomIn,
            KeyBindAction::ZoomOut,
            KeyBindAction::ZoomReset,
        ],
    });
    let mut other_actions = vec![KeyBindAction::ClearScrollback];
    #[cfg(feature = "password_manager")]
    other_actions.push(KeyBindAction::PasswordManager);
    groups.push(ShortcutGroup {
        title: "Other",
        actions: other_actions,
    });
    groups
}

pub fn binding_display(binding: &Binding) -> String {
    binding
        .to_key_bind()
        .map(|key_bind| key_bind.to_string())
        .unwrap_or_else(|| binding.key.clone())
}

pub fn binding_from_key(modifiers: Modifiers, key: Key) -> Option<Binding> {
    if is_modifier_only_key(&key) {
        return None;
    }
    let key = key_to_string(&key)?;
    let mut binding_modifiers = Vec::new();
    if modifiers.control() {
        binding_modifiers.push(ModifierName::Ctrl);
    }
    if modifiers.shift() {
        binding_modifiers.push(ModifierName::Shift);
    }
    if modifiers.alt() {
        binding_modifiers.push(ModifierName::Alt);
    }
    if modifiers.logo() {
        binding_modifiers.push(ModifierName::Super);
    }
    Some(Binding {
        modifiers: binding_modifiers,
        key,
    })
}

fn insert_shortcuts(
    shortcuts: &Shortcuts,
    binds: &mut HashMap<KeyBind, Action>,
    allow_unbind: bool,
) {
    for (binding, action) in &shortcuts.0 {
        let key_bind = match binding.to_key_bind() {
            Some(key_bind) => key_bind,
            None => {
                log::warn!("invalid key binding: {:?}", binding);
                continue;
            }
        };
        if allow_unbind && *action == KeyBindAction::Unbind {
            binds.remove(&key_bind);
            continue;
        }
        let Some(action) = action.to_action() else {
            log::warn!("unsupported shortcut action: {:?}", action);
            continue;
        };
        binds.insert(key_bind, action);
    }
}

fn fallback_shortcuts() -> Shortcuts {
    let mut shortcuts = BTreeMap::new();

    macro_rules! bind {
        ([$($modifier:ident),* $(,)?], $key:expr, $action:ident) => {{
            shortcuts.insert(
                Binding {
                    modifiers: vec![$(ModifierName::$modifier),*],
                    key: $key.to_string(),
                },
                KeyBindAction::$action,
            );
        }};
    }

    // Standard key bindings
    bind!([Ctrl, Shift], "A", SelectAll);
    bind!([Ctrl, Shift], "C", Copy);
    bind!([], "Copy", Copy);
    bind!([Ctrl], "c", CopyOrSigint);
    bind!([Ctrl, Shift], "F", Find);
    bind!([Ctrl, Shift], "N", WindowNew);
    bind!([Ctrl, Shift], "Q", WindowClose);
    bind!([Ctrl, Shift], "T", TabNew);
    bind!([Ctrl, Shift], "V", Paste);
    bind!([], "Paste", Paste);
    bind!([Shift], "Insert", PastePrimary);
    bind!([Ctrl, Shift], "W", TabClose);
    bind!([Ctrl], ",", Settings);
    bind!([], "F11", ToggleFullscreen);

    // Ctrl+Alt+D splits horizontally, Ctrl+Alt+R splits vertically, Ctrl+Shift+X maximizes split
    //TODO: Adjust bindings as desired by UX
    bind!([Ctrl, Alt], "d", PaneSplitHorizontal);
    bind!([Ctrl, Alt], "r", PaneSplitVertical);
    bind!([Ctrl, Shift], "X", PaneToggleMaximized);
    #[cfg(feature = "password_manager")]
    bind!([Ctrl, Alt], "p", PasswordManager);

    // Ctrl+Tab and Ctrl+Shift+Tab cycle through tabs
    // Ctrl+Tab is not a special key for terminals and is free to use
    bind!([Ctrl], "Tab", TabNext);
    bind!([Ctrl, Shift], "Tab", TabPrev);

    // Ctrl+Shift+# activates tabs by index
    bind!([Ctrl, Shift], "1", TabActivate0);
    bind!([Ctrl, Shift], "2", TabActivate1);
    bind!([Ctrl, Shift], "3", TabActivate2);
    bind!([Ctrl, Shift], "4", TabActivate3);
    bind!([Ctrl, Shift], "5", TabActivate4);
    bind!([Ctrl, Shift], "6", TabActivate5);
    bind!([Ctrl, Shift], "7", TabActivate6);
    bind!([Ctrl, Shift], "8", TabActivate7);
    bind!([Ctrl, Shift], "9", TabActivate8);

    // Ctrl+0, Ctrl+-, and Ctrl+= are not special keys for terminals and are free to use
    bind!([Ctrl], "0", ZoomReset);
    bind!([Ctrl], "-", ZoomOut);
    bind!([Ctrl], "=", ZoomIn);
    bind!([Ctrl], "+", ZoomIn);

    // Ctrl+Arrows and Ctrl+HJKL move between splits
    bind!([Ctrl, Shift], "ArrowLeft", PaneFocusLeft);
    bind!([Ctrl, Shift], "H", PaneFocusLeft);
    bind!([Ctrl, Shift], "ArrowDown", PaneFocusDown);
    bind!([Ctrl, Shift], "J", PaneFocusDown);
    bind!([Ctrl, Shift], "ArrowUp", PaneFocusUp);
    bind!([Ctrl, Shift], "K", PaneFocusUp);
    bind!([Ctrl, Shift], "ArrowRight", PaneFocusRight);
    bind!([Ctrl, Shift], "L", PaneFocusRight);

    // CTRL+Alt+L clears the scrollback.
    bind!([Ctrl, Alt], "L", ClearScrollback);

    Shortcuts(shortcuts)
}

fn key_from_string(value: &str) -> Option<Key> {
    match value {
        "Copy" => Some(Key::Named(Named::Copy)),
        "Paste" => Some(Key::Named(Named::Paste)),
        "Insert" => Some(Key::Named(Named::Insert)),
        "Tab" => Some(Key::Named(Named::Tab)),
        "F11" => Some(Key::Named(Named::F11)),
        "ArrowLeft" | "Left" => Some(Key::Named(Named::ArrowLeft)),
        "ArrowRight" | "Right" => Some(Key::Named(Named::ArrowRight)),
        "ArrowUp" | "Up" => Some(Key::Named(Named::ArrowUp)),
        "ArrowDown" | "Down" => Some(Key::Named(Named::ArrowDown)),
        "Space" | "space" => Some(Key::Character(" ".into())),
        _ if !value.is_empty() => Some(Key::Character(value.into())),
        _ => None,
    }
}

fn key_to_string(key: &Key) -> Option<String> {
    match key {
        Key::Character(c) => {
            if c == " " {
                Some("Space".to_string())
            } else if c.len() == 1 && c.chars().all(|ch| ch.is_ascii_alphabetic()) {
                Some(c.to_uppercase())
            } else {
                Some(c.to_string())
            }
        }
        Key::Named(named) => Some(format!("{named:?}")),
        _ => None,
    }
}

fn is_modifier_only_key(key: &Key) -> bool {
    matches!(
        key,
        Key::Named(
            Named::Alt
                | Named::AltGraph
                | Named::CapsLock
                | Named::Control
                | Named::Fn
                | Named::FnLock
                | Named::NumLock
                | Named::ScrollLock
                | Named::Shift
                | Named::Symbol
                | Named::SymbolLock
                | Named::Meta
                | Named::Hyper
                | Named::Super
        )
    )
}
