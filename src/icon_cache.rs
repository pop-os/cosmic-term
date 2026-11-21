// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget::icon;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IconCacheKey {
    name: &'static str,
    size: u16,
}

pub struct IconCache {
    cache: HashMap<IconCacheKey, icon::Handle>,
}

impl IconCache {
    pub fn new() -> Self {
        let mut cache = HashMap::new();

        macro_rules! bundle {
            ($name:expr, $size:expr) => {
                let data: &'static [u8] = include_bytes!(concat!("../res/icons/", $name, ".svg"));
                cache.insert(
                    IconCacheKey {
                        name: $name,
                        size: $size,
                    },
                    icon::from_svg_bytes(data).symbolic(true),
                );
            };
        }

        bundle!("dialog-error-symbolic", 16);
        bundle!("edit-clear-symbolic", 16);
        bundle!("edit-delete-symbolic", 16);
        bundle!("edit-undo-symbolic", 16);
        bundle!("list-add-symbolic", 16);
        bundle!("go-down-symbolic", 16);
        bundle!("go-up-symbolic", 16);
        bundle!("view-more-symbolic", 16);
        bundle!("window-close-symbolic", 16);

        Self { cache }
    }

    pub fn get(&mut self, name: &'static str, size: u16) -> icon::Icon {
        let handle = self
            .cache
            .entry(IconCacheKey { name, size })
            .or_insert_with(|| icon::from_name(name).size(size).handle())
            .clone();
        icon::icon(handle).size(size)
    }
}
