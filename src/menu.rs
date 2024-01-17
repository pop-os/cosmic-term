// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    //TODO: export in cosmic::widget
    iced::{
        widget::{column, horizontal_rule, horizontal_space},
        Alignment, Background, Length,
    },
    theme,
    widget::{
        self,
        menu::{ItemHeight, ItemWidth, MenuBar, MenuTree},
        segmented_button,
    },
    Element,
};

use crate::{fl, Action, Config, ContextPage, Message};

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

pub fn context_menu<'a>(config: &Config, entity: segmented_button::Entity) -> Element<'a, Message> {
    let menu_action = |label, action| {
        menu_button!(widget::text(label)).on_press(Message::TabContextAction(entity, action))
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
        menu_action(fl!("copy"), Action::Copy),
        menu_action(fl!("paste"), Action::Paste),
        menu_action(fl!("select-all"), Action::SelectAll),
        horizontal_rule(1),
        menu_action(fl!("split-horizontal"), Action::PaneSplitHorizontal),
        menu_action(fl!("split-vertical"), Action::PaneSplitVertical),
        menu_action(fl!("pane-toggle-maximize"), Action::PaneToggleMaximized),
        horizontal_rule(1),
        menu_action(fl!("new-tab"), Action::TabNew),
        menu_action(fl!("settings"), Action::Settings),
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
            border_radius: 8.0.into(),
            border_width: 1.0,
            border_color: component.divider.into(),
        }
    }))
    .width(Length::Fixed(240.0))
    .into()
}

pub fn menu_bar<'a>() -> Element<'a, Message> {
    //TODO: port to libcosmic
    let menu_root = |label| {
        widget::button(widget::text(label))
            .padding([4, 12])
            .style(theme::Button::MenuRoot)
    };

    let find_key = |message: &Message| -> String {
        //TODO: hotkey config
        String::new()
    };

    let menu_item = |label, message| {
        let key = find_key(&message);
        MenuTree::new(
            menu_button!(
                widget::text(label),
                horizontal_space(Length::Fill),
                widget::text(key)
            )
            .on_press(message),
        )
    };

    MenuBar::new(vec![
        MenuTree::with_children(
            menu_root(fl!("file")),
            vec![
                menu_item(fl!("new-tab"), Message::TabNew),
                menu_item(fl!("new-window"), Message::WindowNew),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("close-tab"), Message::TabClose(None)),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("quit"), Message::WindowClose),
            ],
        ),
        MenuTree::with_children(
            menu_root(fl!("edit")),
            vec![
                menu_item(fl!("copy"), Message::Copy(None)),
                menu_item(fl!("paste"), Message::Paste(None)),
                menu_item(fl!("select-all"), Message::SelectAll(None)),
                MenuTree::new(horizontal_rule(1)),
                menu_item(fl!("find"), Message::Find(true)),
            ],
        ),
        MenuTree::with_children(
            menu_root(fl!("view")),
            vec![menu_item(
                fl!("menu-settings"),
                Message::ToggleContextPage(ContextPage::Settings),
            )],
        ),
    ])
    .item_height(ItemHeight::Dynamic(40))
    .item_width(ItemWidth::Uniform(240))
    .spacing(4.0)
    .into()
}
