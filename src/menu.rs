// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    //TODO: export in cosmic::widget
    iced::{
        widget::{column, horizontal_rule, horizontal_space},
        Alignment, Background, Length,
    },
    iced_core::Border,
    theme,
    widget::{
        self,
        menu::{ItemHeight, ItemWidth, MenuBar, MenuTree},
        segmented_button,
    },
    Element,
};
use std::collections::HashMap;

use crate::{fl, Action, Config, KeyBind, Message};

macro_rules! menu_button {
    ($($x:expr),+ $(,)?) => (
        widget::button(
            widget::Row::with_children(
                vec![$(Element::from($x)),+]
            )
            .align_items(Alignment::Center)
        )
        .height(Length::Fixed(32.0))
        .padding([4, 16])
        .width(Length::Fill)
        .style(theme::Button::MenuItem)
    );
}

pub fn context_menu<'a>(
    config: &Config,
    key_binds: &HashMap<KeyBind, Action>,
    entity: segmented_button::Entity,
) -> Element<'a, Message> {
    let find_key = |action: &Action| -> String {
        for (key_bind, key_action) in key_binds.iter() {
            if action == key_action {
                return key_bind.to_string();
            }
        }
        String::new()
    };

    let menu_item = |label, action| {
        let key = find_key(&action);
        menu_button!(
            widget::text(label),
            horizontal_space(Length::Fill),
            widget::text(key)
        )
        .on_press(Message::TabContextAction(entity, action))
    };

    let menu_checkbox = |label, value, action| {
        menu_button!(
            widget::text(label),
            widget::horizontal_space(Length::Fill),
            widget::toggler(None, value, move |_| Message::TabContextAction(
                entity, action
            ))
            .size(16.0),
        )
        .on_press(Message::TabContextAction(entity, action))
    };

    widget::container(column!(
        menu_item(fl!("copy"), Action::Copy),
        menu_item(fl!("paste"), Action::Paste),
        menu_item(fl!("select-all"), Action::SelectAll),
        horizontal_rule(1),
        menu_item(fl!("split-horizontal"), Action::PaneSplitHorizontal),
        menu_item(fl!("split-vertical"), Action::PaneSplitVertical),
        menu_item(fl!("pane-toggle-maximize"), Action::PaneToggleMaximized),
        horizontal_rule(1),
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
    .style(theme::Container::custom(|theme| {
        let cosmic = theme.cosmic();
        let component = &cosmic.background.component;
        widget::container::Appearance {
            icon_color: Some(component.on.into()),
            text_color: Some(component.on.into()),
            background: Some(Background::Color(component.base.into())),
            border: Border {
                radius: 8.0.into(),
                width: 1.0,
                color: component.divider.into(),
            },
            ..Default::default()
        }
    }))
    .width(Length::Fixed(240.0))
    .into()
}

pub fn menu_bar<'a>(key_binds: &HashMap<KeyBind, Action>) -> Element<'a, Message> {
    //TODO: port to libcosmic
    let menu_root = |label| {
        widget::button(widget::text(label))
            .padding([4, 12])
            .style(theme::Button::MenuRoot)
    };

    let find_key = |action: &Action| -> String {
        for (key_bind, key_action) in key_binds.iter() {
            if action == key_action {
                return key_bind.to_string();
            }
        }
        String::new()
    };

    let menu_item = |label, action| {
        let key = find_key(&action);
        MenuTree::new(
            menu_button!(
                widget::text(label),
                horizontal_space(Length::Fill),
                widget::text(key)
            )
            .on_press(action.message(None)),
        )
    };

    MenuBar::new(vec![
        MenuTree::with_children(
            menu_root(fl!("file")),
            vec![
                menu_item(fl!("new-tab"), Action::TabNew),
                menu_item(fl!("new-window"), Action::WindowNew),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("close-tab"), Action::TabClose),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("quit"), Action::WindowClose),
            ],
        ),
        MenuTree::with_children(
            menu_root(fl!("edit")),
            vec![
                menu_item(fl!("copy"), Action::Copy),
                menu_item(fl!("paste"), Action::Paste),
                menu_item(fl!("select-all"), Action::SelectAll),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("find"), Action::Find),
            ],
        ),
        MenuTree::with_children(
            menu_root(fl!("view")),
            vec![
                menu_item(fl!("zoom-in"), Action::ZoomIn),
                menu_item(fl!("zoom-reset"), Action::ZoomReset),
                menu_item(fl!("zoom-out"), Action::ZoomOut),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("next-tab"), Action::TabNext),
                menu_item(fl!("previous-tab"), Action::TabPrev),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("split-horizontal"), Action::PaneSplitHorizontal),
                menu_item(fl!("split-vertical"), Action::PaneSplitVertical),
                menu_item(fl!("pane-toggle-maximize"), Action::PaneToggleMaximized),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("menu-settings"), Action::Settings),
            ],
        ),
    ])
    .item_height(ItemHeight::Dynamic(40))
    .item_width(ItemWidth::Uniform(240))
    .spacing(4.0)
    .into()
}
