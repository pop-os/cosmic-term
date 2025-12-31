use cosmic::widget::menu::key_bind::{KeyBind, Modifier};
use cosmic::{iced::keyboard::Key, iced_core::keyboard::key::Named};
use cosmic_config::{CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use ron::value::RawValue;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::collections::HashMap;

use crate::Action;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, CosmicConfigEntry)]
#[version = 1]
pub struct KeyBindConfig {
    #[serde(default)]
    pub keybindings: KeyBindOverrides,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeyBindOverrides(HashMap<KeyBind, BindingAction>);

impl KeyBindOverrides {
    fn iter(&self) -> impl Iterator<Item = (&KeyBind, &BindingAction)> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for KeyBindOverrides {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = HashMap::new();
        for (key_bind, action) in &self.0 {
            if let Some(ron) = KeyBindRon::from_key_bind(key_bind) {
                map.insert(ron, *action);
            }
        }
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyBindOverrides {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<KeyBindRon, BindingAction>::deserialize(deserializer)?;
        let mut result = HashMap::with_capacity(map.len());
        for (ron, action) in map {
            let key_bind = KeyBind::try_from(ron).map_err(D::Error::custom)?;
            result.insert(key_bind, action);
        }
        Ok(Self(result))
    }
}

pub fn key_binds() -> HashMap<KeyBind, Action> {
    default_key_binds()
}

pub fn key_binds_with_overrides(overrides: &KeyBindOverrides) -> HashMap<KeyBind, Action> {
    let mut key_binds = default_key_binds();
    if overrides.is_empty() {
        return key_binds;
    }

    apply_overrides(&mut key_binds, overrides);
    key_binds
}

fn default_key_binds() -> HashMap<KeyBind, Action> {
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
    bind!([Ctrl, Alt], Key::Character("d".into()), PaneSplitHorizontal);
    bind!([Ctrl, Alt], Key::Character("r".into()), PaneSplitVertical);
    bind!(
        [Ctrl, Shift],
        Key::Character("X".into()),
        PaneToggleMaximized
    );
    #[cfg(feature = "password_manager")]
    bind!([Ctrl, Alt], Key::Character("p".into()), PasswordManager);

    // Ctrl+Tab and Ctrl+Shift+Tab cycle through tabs
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

fn apply_overrides(key_binds: &mut HashMap<KeyBind, Action>, overrides: &KeyBindOverrides) {
    for (binding, action) in overrides.iter() {
        match action {
            BindingAction::Action(mapped) => {
                key_binds.insert(binding.clone(), *mapped);
            }
            BindingAction::Disable => {
                key_binds.remove(binding);
            }
        }
    }
}

fn key_to_string(key: &Key) -> Option<String> {
    match key {
        Key::Named(named) => named_to_string(named).map(|s| s.to_string()),
        Key::Character(text) if !text.is_empty() => Some(text.to_string()),
        _ => None,
    }
}

fn string_to_key(name: &str) -> Option<Key> {
    if name.chars().count() == 1 {
        let ch = name.chars().next().unwrap();
        return Some(Key::Character(ch.to_string().into()));
    }
    named_from_string(name).map(Key::Named)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
struct KeyBindRon {
    #[serde(default)]
    modifiers: Vec<ModifierLabel>,
    key: String,
}

impl KeyBindRon {
    fn from_key_bind(key_bind: &KeyBind) -> Option<Self> {
        let mut modifiers = key_bind
            .modifiers
            .iter()
            .map(|m| ModifierLabel::from(*m))
            .collect::<Vec<_>>();
        modifiers.sort_by_key(|label| label.as_str());
        let key = key_to_string(&key_bind.key)?;
        Some(Self { modifiers, key })
    }
}

impl TryFrom<KeyBindRon> for KeyBind {
    type Error = String;

    fn try_from(value: KeyBindRon) -> Result<Self, Self::Error> {
        let modifiers = value
            .modifiers
            .into_iter()
            .map(Modifier::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let key = string_to_key(&value.key).ok_or_else(|| "invalid key".to_string())?;
        Ok(Self { modifiers, key })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum ModifierLabel {
    Super,
    Ctrl,
    Alt,
    Shift,
}

impl ModifierLabel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Super => "Super",
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
        }
    }
}

impl From<Modifier> for ModifierLabel {
    fn from(value: Modifier) -> Self {
        match value {
            Modifier::Super => Self::Super,
            Modifier::Ctrl => Self::Ctrl,
            Modifier::Alt => Self::Alt,
            Modifier::Shift => Self::Shift,
        }
    }
}

impl TryFrom<ModifierLabel> for Modifier {
    type Error = String;

    fn try_from(value: ModifierLabel) -> Result<Self, Self::Error> {
        Ok(match value {
            ModifierLabel::Super => Modifier::Super,
            ModifierLabel::Ctrl => Modifier::Ctrl,
            ModifierLabel::Alt => Modifier::Alt,
            ModifierLabel::Shift => Modifier::Shift,
        })
    }
}

fn named_to_string(named: &Named) -> Option<&'static str> {
    macro_rules! named_map {
        ($($variant:ident),+ $(,)?) => {
            match named {
                $(Named::$variant => Some(stringify!($variant)),)+
                _ => None,
            }
        };
    }

    named_map![
        Insert, Tab, ArrowLeft, ArrowRight, ArrowUp, ArrowDown, PageUp, PageDown, Home, End,
        Delete, Backspace, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16,
        F17, F18, F19, F20, F21, F22, F23, F24, Copy, Paste, Enter, Escape, Space, CapsLock
    ]
}

fn named_from_string(name: &str) -> Option<Named> {
    macro_rules! parse_named {
        ($($variant:ident),+ $(,)?) => {
            match name {
                $(stringify!($variant) => Some(Named::$variant),)+
                _ => None,
            }
        };
    }

    parse_named![
        Insert, Tab, ArrowLeft, ArrowRight, ArrowUp, ArrowDown, PageUp, PageDown, Home, End,
        Delete, Backspace, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16,
        F17, F18, F19, F20, F21, F22, F23, F24, Copy, Paste, Enter, Escape, Space, CapsLock
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) enum BindingAction {
    Action(Action),
    Disable,
}

impl<'de> Deserialize<'de> for BindingAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        let text = raw.trim().get_ron().trim();

        if text == "Disable" {
            return Ok(Self::Disable);
        }

        match ron::from_str::<Action>(text) {
            Ok(action) => Ok(Self::Action(action)),
            Err(err) => Err(D::Error::custom(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ron::de::from_str;

    fn shift_insert_key_bind() -> KeyBind {
        KeyBind {
            modifiers: vec![Modifier::Shift],
            key: Key::Named(Named::Insert),
        }
    }

    #[test]
    fn defaults_include_shift_insert_paste_primary() {
        let binds = key_binds();
        assert_eq!(
            binds.get(&shift_insert_key_bind()),
            Some(&Action::PastePrimary)
        );
    }

    #[test]
    fn overrides_replace_default_binding() {
        let overrides: KeyBindOverrides = from_str(
            r#"{
                (modifiers: [Shift], key: "Insert"): Paste,
            }"#,
        )
        .unwrap();

        let binds = key_binds_with_overrides(&overrides);

        assert_eq!(binds.get(&shift_insert_key_bind()), Some(&Action::Paste));
    }

    #[test]
    fn sample_config_file_parses_and_overrides() {
        const SAMPLE: &str = r#"
        {
            // Use Shift+Insert for Paste instead of PastePrimary.
            (modifiers: [Shift], key: "Insert"): Paste,
        }
        "#;

        let overrides: KeyBindOverrides = from_str(SAMPLE).expect("sample config parses");
        let binds = key_binds_with_overrides(&overrides);

        assert_eq!(binds.get(&shift_insert_key_bind()), Some(&Action::Paste));
    }
}
