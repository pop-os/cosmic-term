// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    //TODO: export in cosmic::widget
    iced::{
        widget::{column, horizontal_rule},
        Alignment, Background, Length,
    },
    theme,
    widget::{self, segmented_button},
    Element,
};

use crate::{fl, Action, Config, Message};

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
