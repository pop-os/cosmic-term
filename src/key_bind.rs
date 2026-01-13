use cosmic::widget::menu::key_bind::KeyBind;
use std::collections::HashMap;

use crate::Action;
use crate::shortcuts::ShortcutsConfig;

pub fn key_binds(shortcuts: &ShortcutsConfig) -> HashMap<KeyBind, Action> {
    shortcuts.key_binds()
}
