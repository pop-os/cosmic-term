// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::widget::menu::{items as menu_items, menu_button, root as menu_root, Item as MenuItem};
use cosmic::{
    iced::{
        widget::{column, horizontal_space},
        Background, Length,
    },
    iced_core::Border,
    widget::{
        self, divider,
        menu::{ItemHeight, ItemWidth, MenuBar, Tree as MenuTree},
        segmented_button,
    },
    Element,
};
use std::collections::HashMap;

use crate::{fl, Action, ColorSchemeId, ColorSchemeKind, Config, Message};

pub fn context_menu<'a>(
    config: &Config,
    key_binds: &HashMap<KeyBind, Action>,
    entity: segmented_button::Entity,
) -> Element<'a, Message> {
    let find_key = |action: &Action| -> String {
        for (key_bind, key_action) in key_binds {
            if action == key_action {
                return key_bind.to_string();
            }
        }
        String::new()
    };

    let menu_item = |label, action| {
        let key = find_key(&action);
        menu_button(vec![
            widget::text(label).into(),
            horizontal_space().into(),
            widget::text(key).into(),
        ])
        .on_press(Message::TabContextAction(entity, action))
    };

    let menu_checkbox = |label, value, action| {
        menu_button(vec![
            widget::text(label).into(),
            widget::horizontal_space().into(),
            widget::toggler(value)
                .on_toggle(move |_| Message::TabContextAction(entity, action))
                .size(16.0)
                .into(),
        ])
        .on_press(Message::TabContextAction(entity, action))
    };

    widget::container(column!(
        menu_item(fl!("copy"), Action::Copy),
        menu_item(fl!("paste"), Action::Paste),
        menu_item(fl!("select-all"), Action::SelectAll),
        divider::horizontal::light(),
        menu_item(fl!("clear-scrollback"), Action::ClearScrollback),
        divider::horizontal::light(),
        menu_item(fl!("split-horizontal"), Action::PaneSplitHorizontal),
        menu_item(fl!("split-vertical"), Action::PaneSplitVertical),
        menu_item(fl!("pane-toggle-maximize"), Action::PaneToggleMaximized),
        divider::horizontal::light(),
        menu_item(fl!("new-tab"), Action::TabNew),
        menu_item(fl!("menu-settings"), Action::Settings),
        menu_checkbox(
            fl!("show-headerbar"),
            config.show_headerbar,
            Action::ShowHeaderBar(!config.show_headerbar)
        ),
    ))
    .padding(1)
    //TODO: move style to libcosmic
    .style(|theme| {
        let cosmic = theme.cosmic();
        let component = &cosmic.background.component;
        widget::container::Style {
            icon_color: Some(component.on.into()),
            text_color: Some(component.on.into()),
            background: Some(Background::Color(component.base.into())),
            border: Border {
                radius: cosmic.radius_s().map(|x| x + 1.0).into(),
                width: 1.0,
                color: component.divider.into(),
            },
            ..Default::default()
        }
    })
    .width(Length::Fixed(240.0))
    .into()
}

pub fn color_scheme_menu<'a>(
    kind: ColorSchemeKind,
    id_opt: Option<ColorSchemeId>,
    name: &str,
) -> Element<'a, Message> {
    let menu_item =
        |label, message| menu_button(vec![widget::text(label).into()]).on_press(message);

    let mut column = widget::column::with_capacity(if id_opt.is_some() { 3 } else { 1 });
    if let Some(id) = id_opt {
        column = column.push(menu_item(
            fl!("rename"),
            Message::ColorSchemeRename(kind, id, name.to_string()),
        ));
    }
    column = column.push(menu_item(
        fl!("export"),
        Message::ColorSchemeExport(kind, id_opt),
    ));
    if let Some(id) = id_opt {
        column = column.push(menu_item(
            fl!("delete"),
            Message::ColorSchemeDelete(kind, id),
        ));
    }

    widget::container(column)
        .padding(1)
        //TODO: move style to libcosmic
        .style(|theme| {
            let cosmic = theme.cosmic();
            let component = &cosmic.background.component;
            widget::container::Style {
                icon_color: Some(component.on.into()),
                text_color: Some(component.on.into()),
                background: Some(Background::Color(component.base.into())),
                border: Border {
                    radius: cosmic.radius_s().map(|x| x + 1.0).into(),
                    width: 1.0,
                    color: component.divider.into(),
                },
                ..Default::default()
            }
        })
        .width(Length::Fixed(120.0))
        .into()
}

pub fn menu_bar<'a>(config: &Config, key_binds: &HashMap<KeyBind, Action>) -> Element<'a, Message> {
    let mut profile_items = Vec::with_capacity(config.profiles.len());
    for (name, id) in config.profile_names() {
        profile_items.push(MenuItem::Button(name, Action::ProfileOpen(id)));
    }

    //TODO: what to do if there are no profiles?

    MenuBar::new(vec![
        MenuTree::with_children(
            menu_root(fl!("file")),
            menu_items(
                key_binds,
                vec![
                    MenuItem::Button(fl!("new-tab"), Action::TabNew),
                    MenuItem::Button(fl!("new-window"), Action::WindowNew),
                    MenuItem::Divider,
                    MenuItem::Folder(fl!("profile"), profile_items),
                    MenuItem::Button(fl!("menu-profiles"), Action::Profiles),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("close-tab"), Action::TabClose),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("quit"), Action::WindowClose),
                ],
            ),
        ),
        MenuTree::with_children(
            menu_root(fl!("edit")),
            menu_items(
                key_binds,
                vec![
                    MenuItem::Button(fl!("copy"), Action::Copy),
                    MenuItem::Button(fl!("paste"), Action::Paste),
                    MenuItem::Button(fl!("select-all"), Action::SelectAll),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("clear-scrollback"), Action::ClearScrollback),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("find"), Action::Find),
                ],
            ),
        ),
        MenuTree::with_children(
            menu_root(fl!("view")),
            menu_items(
                key_binds,
                vec![
                    MenuItem::Button(fl!("zoom-in"), Action::ZoomIn),
                    MenuItem::Button(fl!("zoom-reset"), Action::ZoomReset),
                    MenuItem::Button(fl!("zoom-out"), Action::ZoomOut),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("next-tab"), Action::TabNext),
                    MenuItem::Button(fl!("previous-tab"), Action::TabPrev),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("split-horizontal"), Action::PaneSplitHorizontal),
                    MenuItem::Button(fl!("split-vertical"), Action::PaneSplitVertical),
                    MenuItem::Button(fl!("pane-toggle-maximize"), Action::PaneToggleMaximized),
                    MenuItem::Divider,
                    MenuItem::Button(
                        fl!("menu-color-schemes"),
                        Action::ColorSchemes(config.color_scheme_kind()),
                    ),
                    MenuItem::Button(fl!("menu-settings"), Action::Settings),
                    MenuItem::Divider,
                    MenuItem::Button(fl!("menu-about"), Action::About),
                ],
            ),
        ),
    ])
    .item_height(ItemHeight::Dynamic(40))
    .item_width(ItemWidth::Uniform(240))
    .spacing(4.0)
    .into()
}
