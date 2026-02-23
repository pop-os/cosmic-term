// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Point;
use cosmic::widget::Column;
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::widget::menu::{Item as MenuItem, menu_button};
use cosmic::{
    Element,
    app::Core,
    iced::{
        Background, Length, advanced::widget::text::Style as TextStyle, widget::horizontal_space,
    },
    iced_core::Border,
    theme,
    widget::{
        self, divider,
        menu::{ItemHeight, ItemWidth},
        responsive_menu_bar, segmented_button,
    },
};
use std::{collections::HashMap, sync::LazyLock};

use crate::{Action, ColorSchemeId, ColorSchemeKind, Config, Message, fl};

static MENU_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::new("responsive-menu"));

#[derive(Debug, Clone)]
pub struct MenuState {
    pub position: Option<Point>,
    pub link: Option<String>,
}

pub fn context_menu<'a>(
    config: &Config,
    key_binds: &HashMap<KeyBind, Action>,
    entity: segmented_button::Entity,
    link: Option<String>,
) -> Element<'a, Message> {
    let find_key = |action: &Action| -> String {
        for (key_bind, key_action) in key_binds {
            if action == key_action {
                return key_bind.to_string();
            }
        }
        String::new()
    };
    fn key_style(theme: &cosmic::Theme) -> TextStyle {
        let mut color = theme.cosmic().background.component.on;
        color.alpha *= 0.75;
        TextStyle {
            color: Some(color.into()),
        }
    }

    let menu_item = |label, action| {
        let key = find_key(&action);
        menu_button(vec![
            widget::text(label).into(),
            horizontal_space().into(),
            widget::text(key)
                .class(theme::Text::Custom(key_style))
                .into(),
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

    let mut rows = vec![
        Element::from(menu_item(fl!("copy"), Action::Copy)),
        Element::from(menu_item(fl!("paste"), Action::Paste)),
        Element::from(menu_item(fl!("select-all"), Action::SelectAll)),
        Element::from(divider::horizontal::light()),
        Element::from(menu_item(fl!("clear-scrollback"), Action::ClearScrollback)),
        Element::from(divider::horizontal::light()),
        Element::from(menu_item(
            fl!("split-horizontal"),
            Action::PaneSplitHorizontal,
        )),
        Element::from(menu_item(fl!("split-vertical"), Action::PaneSplitVertical)),
        Element::from(menu_item(
            fl!("pane-toggle-maximize"),
            Action::PaneToggleMaximized,
        )),
        Element::from(divider::horizontal::light()),
        Element::from(menu_item(fl!("new-tab"), Action::TabNew)),
        Element::from(menu_item(fl!("menu-settings"), Action::Settings)),
    ];
    #[cfg(feature = "password_manager")]
    {
        rows.push(Element::from(menu_item(
            fl!("menu-password-manager"),
            Action::PasswordManager,
        )));
    }
    rows.push(Element::from(menu_checkbox(
        fl!("show-headerbar"),
        config.show_headerbar,
        Action::ShowHeaderBar(!config.show_headerbar),
    )));

    //If we have a link
    //prepend the Open Link item
    if link.is_some() {
        rows.insert(
            0,
            Element::from(menu_item(fl!("open-link"), Action::LaunchUrlByMenu)),
        );
        rows.insert(
            1,
            Element::from(menu_item(fl!("copy-link"), Action::CopyUrlByMenu)),
        );
        rows.insert(2, Element::from(divider::horizontal::light()));
    }
    let content = Column::with_children(rows);
    widget::container(content)
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
        .width(Length::Fixed(360.0))
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

pub fn menu_bar<'a>(
    core: &Core,
    config: &Config,
    key_binds: &HashMap<KeyBind, Action>,
) -> Element<'a, Message> {
    let mut profile_items = Vec::with_capacity(config.profiles.len());
    for (name, id) in config.profile_names() {
        profile_items.push(MenuItem::Button(name, None, Action::ProfileOpen(id)));
    }

    //TODO: what to do if there are no profiles?

    responsive_menu_bar()
        .item_height(ItemHeight::Dynamic(40))
        .item_width(ItemWidth::Uniform(320))
        .spacing(4.0)
        .into_element(
            core,
            key_binds,
            MENU_ID.clone(),
            Message::Surface,
            vec![
                (
                    fl!("file"),
                    vec![
                        MenuItem::Button(fl!("new-tab"), None, Action::TabNew),
                        MenuItem::Button(fl!("new-window"), None, Action::WindowNew),
                        MenuItem::Divider,
                        MenuItem::Folder(fl!("profile"), profile_items),
                        MenuItem::Button(fl!("menu-profiles"), None, Action::Profiles),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("close-tab"), None, Action::TabClose),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("quit"), None, Action::WindowClose),
                    ],
                ),
                (
                    fl!("edit"),
                    vec![
                        MenuItem::Button(fl!("copy"), None, Action::Copy),
                        MenuItem::Button(fl!("paste"), None, Action::Paste),
                        MenuItem::Button(fl!("select-all"), None, Action::SelectAll),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("clear-scrollback"), None, Action::ClearScrollback),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("find"), None, Action::Find),
                    ],
                ),
                (
                    fl!("view"),
                    vec![
                        MenuItem::Button(fl!("zoom-in"), None, Action::ZoomIn),
                        MenuItem::Button(fl!("zoom-reset"), None, Action::ZoomReset),
                        MenuItem::Button(fl!("zoom-out"), None, Action::ZoomOut),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("next-tab"), None, Action::TabNext),
                        MenuItem::Button(fl!("previous-tab"), None, Action::TabPrev),
                        MenuItem::Divider,
                        MenuItem::Button(
                            fl!("split-horizontal"),
                            None,
                            Action::PaneSplitHorizontal,
                        ),
                        MenuItem::Button(fl!("split-vertical"), None, Action::PaneSplitVertical),
                        MenuItem::Button(
                            fl!("pane-toggle-maximize"),
                            None,
                            Action::PaneToggleMaximized,
                        ),
                        MenuItem::Divider,
                        MenuItem::Button(
                            fl!("menu-color-schemes"),
                            None,
                            Action::ColorSchemes(config.color_scheme_kind()),
                        ),
                        MenuItem::Button(
                            fl!("menu-keyboard-shortcuts"),
                            None,
                            Action::KeyboardShortcuts,
                        ),
                        MenuItem::Button(fl!("menu-settings"), None, Action::Settings),
                        #[cfg(feature = "password_manager")]
                        MenuItem::Button(
                            fl!("menu-password-manager"),
                            None,
                            Action::PasswordManager,
                        ),
                        MenuItem::Divider,
                        MenuItem::Button(fl!("menu-about"), None, Action::About),
                    ],
                ),
            ],
        )
}
