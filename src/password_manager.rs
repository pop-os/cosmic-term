use cosmic::{iced::Length, widget, Element};

use crate::{fl, Message};

pub struct PasswordManager {
    pub input_description: String,
    pub input_password: String,
    pub cached_password_list: Option<Vec<String>>,
}

impl PasswordManager {
    pub fn new() -> Self {
        Self {
            input_description: Default::default(),
            input_password: Default::default(),
            cached_password_list: None,
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
        let ss = match secret_service::blocking::SecretService::connect(
            secret_service::EncryptionType::Dh,
        ) {
            Ok(ss) => ss,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return None;
            }
        };
        let collection = match ss.get_default_collection() {
            Ok(c) => c,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return None;
            }
        };

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", identifier);

        let search_items = match collection.search_items(attributes) {
            Ok(sr) => sr,
            Err(err) => {
                log::error!("Failed to search for passwords: {err}");
                return None;
            }
        };

        if let Some(Ok(secret)) = search_items.first().map(|item| item.get_secret()) {
            return String::from_utf8(secret).ok();
        }
        return None;
    }

    pub fn get_password_list(&self) -> Vec<String> {
        let mut list = Vec::new();
        let ss = match secret_service::blocking::SecretService::connect(
            secret_service::EncryptionType::Dh,
        ) {
            Ok(ss) => ss,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return list;
            }
        };
        let collection = match ss.get_default_collection() {
            Ok(c) => c,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return list;
            }
        };

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");

        let search_items = match collection.search_items(attributes) {
            Ok(sr) => sr,
            Err(err) => {
                log::error!("Failed to search for passwords: {err}");
                return list;
            }
        };

        list = search_items
            .iter()
            .flat_map(|item| {
                item.get_attributes()
                    .ok()
                    .and_then(|attribs| attribs.get("identifier").cloned())
            })
            .collect();

        list.sort();
        list
    }

    pub fn delete_password(&mut self, identifier: &str) {
        let ss = match secret_service::blocking::SecretService::connect(
            secret_service::EncryptionType::Dh,
        ) {
            Ok(ss) => ss,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return;
            }
        };
        let collection = match ss.get_default_collection() {
            Ok(c) => c,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return;
            }
        };

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", identifier);

        let search_items = match collection.search_items(attributes) {
            Ok(sr) => sr,
            Err(err) => {
                log::error!("Failed to search for passwords: {err}");
                return;
            }
        };

        if let Some(item) = search_items.first() {
            if let Err(err) = item.delete() {
                log::error!("Failed to delete password {identifier}: {err}");
            }
        }
        self.clear_password_cache();
        self.populate_password_cache();
    }

    pub fn add_inputed_password(&mut self) {
        let ss = match secret_service::blocking::SecretService::connect(
            secret_service::EncryptionType::Dh,
        ) {
            Ok(ss) => ss,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return;
            }
        };
        let collection = match ss.get_default_collection() {
            Ok(c) => c,
            Err(err) => {
                log::error!("Failed to connect to Secret Service: {err}");
                return;
            }
        };

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.system76.CosmicTerm");
        attributes.insert("identifier", &self.input_description);

        let label = format!("CosmicTerm - {}", self.input_description);

        if let Err(err) = collection.create_item(
            &label,
            attributes,
            self.input_password.as_bytes(),
            true,
            "text/plain",
        ) {
            log::error!("Failed to store password in secret service: {}", err);
            return;
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
