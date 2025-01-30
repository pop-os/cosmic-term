use cosmic::widget::menu::key_bind::{KeyBind, Modifier};
use cosmic::{iced::keyboard::Key, iced_core::keyboard::key::Named};
use home::home_dir;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;


use crate::Action;

#[derive(Debug, Deserialize, Serialize)]
struct RawCustomConfig {
    key_bindings: Vec<RawCustomKeyBinding>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawCustomKeyBinding {
    key: String,
    mods: String,
    action: String,
}

//TODO: this function is called way too often
pub fn key_binds() -> HashMap<KeyBind, Action> {
    let mut key_binds: HashMap<KeyBind, Action> = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),+ $(,)?], $key:expr, $action:ident) => {{
            key_binds.insert(
                KeyBind {
                    modifiers: vec![$(Modifier::$modifier),+],
                    key: $key,
                },
                Action::$action,
            );
        }};
    }

    // Standard key bindings
    bind!([Ctrl, Shift], Key::Character("A".into()), SelectAll);
    bind!([Ctrl, Shift], Key::Character("C".into()), Copy);
    bind!([Ctrl], Key::Character("c".into()), CopyOrSigint);
    bind!([Ctrl, Shift], Key::Character("F".into()), Find);
    bind!([Ctrl, Shift], Key::Character("N".into()), WindowNew);
    bind!([Ctrl, Shift], Key::Character("Q".into()), WindowClose);
    bind!([Ctrl, Shift], Key::Character("T".into()), TabNew);
    bind!([Ctrl, Shift], Key::Character("V".into()), Paste);
    bind!([Shift], Key::Named(Named::Insert), PastePrimary);
    bind!([Ctrl, Shift], Key::Character("W".into()), TabClose);
    bind!([Ctrl], Key::Character(",".into()), Settings);

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

    // overwrite defaults

    let raw_config: RawCustomConfig;

    if let Some(home) = home_dir() {
        let path = home
            .join(".config")
            .join("cosmic-term")
            .join("cosmic-term.yaml");

        match load_config(path) {
            Ok(config) => raw_config = config,

            Err(e) => {
                log::error!("error loading keybind config: {:?}", e);
                return key_binds;
            }
        }
    } else {
        return key_binds;
    }

    // for now only allow to overwrite actions that already have a key bind by default
    let mut valid_actions: HashMap<String, Action> = HashMap::new();
    for value in key_binds.values() {
        valid_actions.insert(format!("{value:?}"), *value);
    }

    'outer: for raw_key_binding in raw_config.key_bindings.iter() {
        if !(valid_actions.contains_key(&raw_key_binding.action)) {
            continue;
        }
        // TODO: use named keys
        if raw_key_binding.key.chars().count() != 1 {
            continue;
        }

        let mut custom_key_bind = KeyBind {
            modifiers: vec![],
            key: Key::Character(raw_key_binding.key.as_str().into()),
        };

        for modifier in raw_key_binding.mods.split('|') {
            match modifier.to_lowercase().as_str() {
                "alt" => custom_key_bind.modifiers.push(Modifier::Alt),
                "ctrl" => custom_key_bind.modifiers.push(Modifier::Ctrl),
                "shift" => custom_key_bind.modifiers.push(Modifier::Shift),
                "super" => custom_key_bind.modifiers.push(Modifier::Super),
                _ => continue 'outer,
            }
        }
        key_binds.insert(custom_key_bind, valid_actions[&raw_key_binding.action]);
    }

    key_binds
}

fn load_config(path: PathBuf) -> Result<RawCustomConfig, Box<dyn std::error::Error>> {
    let content: String = fs::read_to_string(path)?; // Read file into a string
    let config: RawCustomConfig = serde_yaml::from_str(&content)?; // Deserialize YAML into rust struct
    Ok(config)
}
