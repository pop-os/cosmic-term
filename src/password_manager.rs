use cosmic::{iced::Length, widget, Element};
use libsecret::prelude::RetrievableExtManual;

use crate::{fl, Message};

pub struct PasswordManager {
    pub input_description: String,
    pub input_password: String,
    pub cached_password_list: Option<Vec<String>>,
    schema: libsecret::Schema,
}

impl PasswordManager {
    pub fn new() -> Self {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", libsecret::SchemaAttributeType::String);
        attributes.insert("identifier", libsecret::SchemaAttributeType::String);
        let schema = libsecret::Schema::new(
            "com.system76.CosmicTerm",
            libsecret::SchemaFlags::NONE,
            attributes,
        );
        Self {
            input_description: Default::default(),
            input_password: Default::default(),
            cached_password_list: None,
            schema,
        }
    }

    pub fn populate_password_cache(&mut self) {
        if self.cached_password_list.is_none() {
            self.cached_password_list = Some(self.get_password_list());
        }
    }

    pub fn clear_password_cache(&mut self) {
        self.cached_password_list = None;
    }

    pub fn get_password(&self, identifier: &str) -> Option<String> {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", identifier);

        libsecret::password_lookup_sync(Some(&self.schema), attributes, gio::Cancellable::NONE)
            .map_or(None, |result| result.map(|s| s.to_string()))
    }

    pub fn get_password_list(&self) -> Vec<String> {
        let mut list = Vec::new();
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        match libsecret::password_search_sync(
            Some(&self.schema),
            attributes,
            libsecret::SearchFlags::ALL,
            gio::Cancellable::NONE,
        ) {
            Ok(passwords) => {
                for r in passwords {
                    if let Some(label) = r.attributes().get("identifier").cloned() {
                        list.push(label);
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to list password: {err}");
            }
        };
        list.sort();
        list
    }

    pub fn delete_password(&mut self, identifier: &str) {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", identifier);
        if let Err(err) =
            libsecret::password_clear_sync(Some(&self.schema), attributes, gio::Cancellable::NONE)
        {
            log::error!("Failed to delete password {identifier}: {err}");
        }
        self.clear_password_cache();
        self.populate_password_cache();
    }

    pub fn add_inputed_password(&mut self) {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", &self.input_description);

        let label = format!("CosmicTerm - {}", self.input_description);

        let collection = libsecret::COLLECTION_DEFAULT;

        let res = libsecret::password_store_sync(
            Some(&self.schema),
            attributes,
            Some(collection),
            &label,
            &self.input_password,
            gio::Cancellable::NONE,
        );
        if let Err(err) = res {
            log::error!("Failed to store password: {}", err);
        }
        if let Some(password_list) = self.cached_password_list.as_mut() {
            password_list.push(self.input_description.clone());
            password_list.sort();
        }
        self.input_description.clear();
        self.input_password.clear();
    }

    pub fn context_page(&self) -> Element<Message> {
        let fresh_password_list;
        let passwords = if let Some(cached_password_list) = self.cached_password_list.as_ref() {
            cached_password_list
        } else {
            fresh_password_list = self.get_password_list();
            &fresh_password_list
        };
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        let mut column = widget::list::list_column();
        for label in passwords {
            column = column.add(
                widget::row()
                    .width(Length::Fill)
                    .push(
                        widget::button::text(label.clone())
                            .width(Length::Fill)
                            .on_press(Message::Password(label.clone())),
                    )
                    .push(widget::horizontal_space(Length::Fill))
                    .push(
                        widget::button::text("-").on_press(Message::PasswordDelete(label.clone())),
                    ),
            );
        }

        //Not really settings, but it seems this does what I want.
        let passwords_view = widget::settings::view_section(fl!("passwords")).add(column);
        let add_password_view = widget::settings::view_section(fl!("add-password"))
            .add(
                widget::text_input::text_input("Description", &self.input_description)
                    .on_input(Message::PasswordDescriptionSubmit),
            )
            .add(
                widget::text_input::text_input("Password", &self.input_password)
                    .on_input(Message::PasswordValueSubmit),
            )
            .add(widget::button::text("Add").on_press(Message::PasswordAdd));

        widget::settings::view_column(vec![
            passwords_view.into(),
            widget::horizontal_space(Length::Fill).into(),
            add_password_view.into(),
        ])
        .into()
    }
}

impl Default for PasswordManager {
    fn default() -> Self {
        Self::new()
    }
}
