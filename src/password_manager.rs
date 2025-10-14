use cosmic::{
    Element, Task, Theme, cosmic_theme,
    iced::{Alignment, Length, Padding},
    style,
    widget::{self, settings::Section},
};

use crate::{Message, fl, icon_cache_get};

#[derive(Clone, Debug)]
pub enum PasswordManagerMessage {
    Error(String),
    FetchAndPastePassword(String),
    FetchAndExpand(String),
    Collapse,
    Delete(String),
    Expand(String, secstr::SecUtf8),
    New,
    RefreshList,
    ToggleShowPassword,
    ListRefreshed(Vec<String>),
    DescriptionInput(String),
    DescriptionInputAndUpdate(String),
    PasswordInput(String),
    PasswordInputAndUpdate(String),
    Update,
    None,
}

struct PasswordInputState {
    pub original: Option<InputState>,
    pub input: InputState,
    pub show_password: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct InputState {
    pub identifier: String,
    pub password: String,
}

pub struct PasswordManager {
    input_state: Option<PasswordInputState>,
    pub password_list: Vec<String>,
    //Which pane we should paste to, ie. which pane had focus when the
    //password manager was opened. Just to be sure it doesn't change under
    //our feet.
    pub pane: Option<widget::pane_grid::Pane>,
    pub expanded_entry: Option<String>,
}

impl PasswordManager {
    pub fn new() -> Self {
        Self {
            input_state: None,
            password_list: Default::default(),
            pane: None,
            expanded_entry: None,
        }
    }

    pub fn update(&mut self, msg: PasswordManagerMessage) -> Task<cosmic::Action<Message>> {
        match msg {
            PasswordManagerMessage::Error(err) => {
                log::error!("{err}");
            }
            PasswordManagerMessage::FetchAndPastePassword(identifier) => {
                return self.fetch_and_paste(identifier);
            }
            PasswordManagerMessage::FetchAndExpand(identifier) => {
                return self.fetch_and_expand(identifier);
            }
            PasswordManagerMessage::Delete(identifier) => {
                return self.delete_password(identifier);
            }
            PasswordManagerMessage::RefreshList => {
                return self.refresh_password_list();
            }
            PasswordManagerMessage::ListRefreshed(list) => {
                self.password_list = list;
            }
            PasswordManagerMessage::Collapse => {
                self.expanded_entry = None;
                self.input_state = None;
                self.expanded_entry = None;
            }
            PasswordManagerMessage::Expand(identifier, password) => {
                self.input_state = Some(PasswordInputState {
                    original: Some(InputState {
                        identifier: identifier.clone(),
                        password: password.clone().into_unsecure(),
                    }),
                    input: InputState {
                        identifier: identifier.clone(),
                        password: password.into_unsecure(),
                    },
                    show_password: false,
                });
                self.expanded_entry = Some(identifier);
            }
            PasswordManagerMessage::ToggleShowPassword => {
                if let Some(input_state) = self.input_state.as_mut() {
                    input_state.show_password = !input_state.show_password;
                }
            }
            PasswordManagerMessage::DescriptionInput(description) => {
                if let Some(input_state) = self.input_state.as_mut() {
                    input_state.input.identifier = description;
                }
            }
            PasswordManagerMessage::DescriptionInputAndUpdate(description) => {
                if let Some(input_state) = self.input_state.as_mut() {
                    input_state.input.identifier = description.clone();
                    return self.add_or_update_password_entry();
                }
            }
            PasswordManagerMessage::PasswordInput(password) => {
                if let Some(input_state) = self.input_state.as_mut() {
                    input_state.input.password = password;
                }
            }
            PasswordManagerMessage::PasswordInputAndUpdate(password) => {
                if let Some(input_state) = self.input_state.as_mut() {
                    input_state.input.password = password;
                    return self.add_or_update_password_entry();
                }
            }
            PasswordManagerMessage::Update => {
                return self.add_or_update_password_entry();
            }
            PasswordManagerMessage::New => {
                self.new_password();
            }
            PasswordManagerMessage::None => {}
        }
        Task::none()
    }

    pub fn clear(&mut self) {
        self.input_state = None;
        self.password_list.clear();
        self.pane = None;
        self.expanded_entry = None;
    }

    pub fn fetch_and_paste(&self, identifier: String) -> Task<cosmic::Action<Message>> {
        if let Some(pane) = self.pane {
            cosmic::task::future(async move {
                match store::get_password(identifier.clone()).await {
                    Ok(password) => Message::PasswordPaste(password, pane),
                    Err(err) => Message::PasswordManager(PasswordManagerMessage::Error(format!(
                        "Failed to fetch password {identifier}: {err}"
                    ))),
                }
            })
        } else {
            log::error!("No active pane set for password manager to use");
            Task::none()
        }
    }

    pub fn fetch_and_expand(&mut self, identifier: String) -> Task<cosmic::Action<Message>> {
        cosmic::task::future(async move {
            match store::get_password(identifier.clone()).await {
                Ok(password) => {
                    Message::PasswordManager(PasswordManagerMessage::Expand(identifier, password))
                }
                Err(err) => Message::PasswordManager(PasswordManagerMessage::Error(format!(
                    "Failed to fetch password {identifier}: {err}"
                ))),
            }
        })
    }

    pub fn refresh_password_list(&self) -> Task<cosmic::Action<Message>> {
        cosmic::task::future(async {
            match store::fetch_password_list().await {
                Ok(list) => Message::PasswordManager(PasswordManagerMessage::ListRefreshed(list)),
                Err(err) => Message::PasswordManager(PasswordManagerMessage::Error(format!(
                    "Failed to fetch password list: {err}"
                ))),
            }
        })
    }

    pub fn delete_password(&mut self, identifier: String) -> Task<cosmic::Action<Message>> {
        if self.expanded_entry.as_ref() == Some(&identifier) {
            self.expanded_entry = None;
        }
        cosmic::task::future(async move {
            if let Err(err) = store::delete_password(identifier.clone()).await {
                return Message::PasswordManager(PasswordManagerMessage::Error(format!(
                    "Failed to delete password {identifier}: {err}"
                )));
            }
            match store::fetch_password_list().await {
                Ok(list) => Message::PasswordManager(PasswordManagerMessage::ListRefreshed(list)),
                Err(err) => Message::PasswordManager(PasswordManagerMessage::Error(format!(
                    "Failed to fetch password list: {err}"
                ))),
            }
        })
    }

    pub fn add_or_update_password_entry(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(input_state) = &self.input_state
            && !input_state.input.identifier.is_empty()
        {
            let original = input_state.original.clone();
            let identifier = input_state.input.identifier.clone();
            let password = input_state.input.password.clone();
            let expanded_identifier = input_state
                .original
                .as_ref()
                .map(|i| i.identifier.clone())
                .unwrap_or(String::new());

            // Ensure we have a non-empty identifier
            if identifier.is_empty() {
                return Task::none();
            }

            // If the identifier have changed, we need to update
            // the password list and expand the new id
            if expanded_identifier != identifier {
                self.expanded_entry = Some(identifier.clone());
                if let Some(i) = self
                    .password_list
                    .iter()
                    .position(|s| s == &expanded_identifier)
                {
                    self.password_list[i] = identifier.clone();
                }
            }

            // Don't do anything if nothing have changed
            if let Some(original) = &original {
                if original == &input_state.input {
                    return Task::none();
                }
            }

            cosmic::task::future(async move {
                if let Err(err) = store::add_password(identifier.clone(), password.clone()).await {
                    Message::PasswordManager(PasswordManagerMessage::Error(format!(
                        "Failed to add password {identifier}: {err}"
                    )))
                } else {
                    if let Some(original) = original {
                        if original.identifier != identifier {
                            if let Err(err) =
                                store::delete_password(original.identifier.clone()).await
                            {
                                return Message::PasswordManager(PasswordManagerMessage::Error(
                                    format!(
                                        "Failed to delete password {}: {err}",
                                        original.identifier
                                    ),
                                ));
                            }
                        }
                    }
                    Message::PasswordManager(PasswordManagerMessage::None)
                }
            })
        } else {
            Task::none()
        }
    }

    pub fn context_page(&self, theme: &Theme) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_xs,
            space_xxs,
            space_xxxs,
            ..
        } = theme.cosmic().spacing;

        let mut sections = Vec::with_capacity(2);

        let mut passwords_section = widget::settings::section();

        for password_id in &self.password_list {
            let expanded = self.expanded_entry.as_ref() == Some(password_id);

            passwords_section = passwords_section.add(
                widget::settings::item::item_row(vec![
                    widget::button::text(password_id.clone())
                        .width(Length::Fixed(290.0))
                        .on_press(Message::PasswordManager(
                            PasswordManagerMessage::FetchAndPastePassword(password_id.clone()),
                        ))
                        .into(),
                    widget::button::custom(icon_cache_get("edit-delete-symbolic", 16))
                        .on_press(Message::PasswordManager(PasswordManagerMessage::Delete(
                            password_id.clone(),
                        )))
                        .class(style::Button::Icon)
                        .into(),
                    if expanded {
                        widget::button::custom(icon_cache_get("go-up-symbolic", 16))
                            .on_press(Message::PasswordManager(PasswordManagerMessage::Collapse))
                    } else {
                        widget::button::custom(icon_cache_get("go-down-symbolic", 16)).on_press(
                            Message::PasswordManager(PasswordManagerMessage::FetchAndExpand(
                                password_id.clone(),
                            )),
                        )
                    }
                    .class(style::Button::Icon)
                    .into(),
                ])
                .align_y(Alignment::Center)
                .spacing(space_xxs),
            );

            if expanded {
                if let Some(input_state) = &self.input_state {
                    let expanded_section: Section<'_, Message> = widget::settings::section().add(
                        widget::column::with_children(vec![
                            widget::column::with_children(vec![
                                widget::text(fl!("password-input-description")).into(),
                                widget::text_input("", input_state.input.identifier.clone())
                                    .on_input(move |text| {
                                        Message::PasswordManager(
                                            PasswordManagerMessage::DescriptionInput(text),
                                        )
                                    })
                                    .on_submit(move |text| {
                                        Message::PasswordManager(
                                            PasswordManagerMessage::DescriptionInputAndUpdate(text),
                                        )
                                    })
                                    .on_unfocus(Message::PasswordManager(
                                        PasswordManagerMessage::Update,
                                    ))
                                    .into(),
                            ])
                            .spacing(space_xxxs)
                            .into(),
                            widget::column::with_children(vec![
                                widget::text(fl!("password-input")).into(),
                                widget::secure_input(
                                    "",
                                    input_state.input.password.clone(),
                                    Some(Message::PasswordManager(
                                        PasswordManagerMessage::ToggleShowPassword,
                                    )),
                                    !input_state.show_password,
                                )
                                .on_input(move |text| {
                                    Message::PasswordManager(PasswordManagerMessage::PasswordInput(
                                        text,
                                    ))
                                })
                                .on_submit(move |text| {
                                    Message::PasswordManager(
                                        PasswordManagerMessage::PasswordInputAndUpdate(text),
                                    )
                                })
                                .on_unfocus(Message::PasswordManager(
                                    PasswordManagerMessage::Update,
                                ))
                                .into(),
                            ])
                            .spacing(space_xxxs)
                            .into(),
                        ])
                        .padding([0, space_s])
                        .spacing(space_xs),
                    );

                    let padding = Padding {
                        top: 0.0,
                        bottom: 0.0,
                        left: space_s.into(),
                        right: space_s.into(),
                    };

                    passwords_section =
                        passwords_section.add(widget::container(expanded_section).padding(padding))
                }
            }
        }
        sections.push(passwords_section.into());

        let add_password = widget::row::with_children(vec![
            widget::horizontal_space().into(),
            widget::button::standard(fl!("add-password"))
                .on_press(Message::PasswordManager(PasswordManagerMessage::New))
                .into(),
        ]);
        sections.push(add_password.into());

        widget::settings::view_column(sections).into()
    }

    pub fn new_password(&mut self) {
        if !self.password_list.contains(&"".to_string()) {
            self.password_list.push("".to_string());
        }
        self.input_state = Some(PasswordInputState {
            original: None,
            input: InputState {
                identifier: Default::default(),
                password: Default::default(),
            },
            show_password: true,
        });
        self.expanded_entry = Some("".to_string());
    }
}

impl Default for PasswordManager {
    fn default() -> Self {
        Self::new()
    }
}

mod store {
    use std::string::FromUtf8Error;

    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum Error {
        #[error(transparent)]
        SecretService(#[from] secret_service::Error),
        #[error(transparent)]
        FromUtf8(#[from] FromUtf8Error),
        #[error("No password found for identifier `{0}`")]
        NoPasswordForIdentifier(String),
    }

    pub async fn fetch_password_list() -> Result<Vec<String>, Error> {
        let mut list = Vec::new();
        let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
        let collection = ss.get_default_collection().await?;

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");

        let search_items = collection.search_items(attributes).await?;

        for item in search_items {
            if let Some(identity) = item
                .get_attributes()
                .await
                .ok()
                .and_then(|attribs| attribs.get("identifier").cloned())
            {
                list.push(identity);
            }
        }
        list.sort();
        Ok(list)
    }

    pub async fn add_password(identifier: String, password: String) -> Result<(), Error> {
        let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
        let collection = ss.get_default_collection().await?;

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", &identifier);

        let label = format!("CosmicTerm - {}", identifier);

        collection
            .create_item(&label, attributes, password.as_bytes(), true, "text/plain")
            .await?;
        Ok(())
    }

    pub async fn get_password(identifier: String) -> Result<secstr::SecUtf8, Error> {
        let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
        let collection = ss.get_default_collection().await?;

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", &identifier);

        let search_items = collection.search_items(attributes).await?;
        if let Some(item) = search_items.first() {
            let secret = item.get_secret().await?;
            Ok(String::from_utf8(secret)?.into())
        } else {
            Err(Error::NoPasswordForIdentifier(identifier))
        }
    }

    pub async fn delete_password(identifier: String) -> Result<(), Error> {
        let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
        let collection = ss.get_default_collection().await?;

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", &identifier);

        let search_items = collection.search_items(attributes).await?;

        if let Some(item) = search_items.first() {
            Ok(item.delete().await?)
        } else {
            Err(Error::NoPasswordForIdentifier(identifier))
        }
    }
}
