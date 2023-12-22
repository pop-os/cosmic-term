// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use alacritty_terminal::{
    config::Config as TermConfig, event::Event as TermEvent, term::color::Colors as TermColors, tty,
};
use cosmic::{
    app::{message, Command, Core, Settings},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        advanced::graphics::text::font_system,
        clipboard, event,
        futures::SinkExt,
        keyboard::{Event as KeyEvent, KeyCode, Modifiers},
        subscription::{self, Subscription},
        widget::row,
        window, Alignment, Event, Length, Point,
    },
    style,
    widget::{self, segmented_button},
    Application, ApplicationExt, Element,
};
use cosmic_text::Family;
use std::{any::TypeId, collections::HashMap, process, sync::Mutex};
use tokio::sync::mpsc;

use config::{AppTheme, Config, CONFIG_VERSION};
mod config;

mod localize;

mod menu;

use self::terminal::{Terminal, TerminalScroll};
mod terminal;

use self::terminal_box::terminal_box;
mod terminal_box;

mod terminal_theme;

/// Runs application with these settings
#[rustfmt::skip]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(unix, not(target_os = "redox")))]
    match fork::daemon(true, true) {
        Ok(fork::Fork::Child) => (),
        Ok(fork::Fork::Parent(_child_pid)) => process::exit(0),
        Err(err) => {
            eprintln!("failed to daemonize: {:?}", err);
            process::exit(1);
        }
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    localize::localize();

    let (config_handler, config) = match cosmic_config::Config::new(App::APP_ID, CONFIG_VERSION) {
        Ok(config_handler) => {
            let config = match Config::get_entry(&config_handler) {
                Ok(ok) => ok,
                Err((errs, config)) => {
                    log::info!("errors loading config: {:?}", errs);
                    config
                }
            };
            (Some(config_handler), config)
        }
        Err(err) => {
            log::error!("failed to create config handler: {}", err);
            (None, Config::default())
        }
    };

    // Set up environmental variables for terminal
    let mut term_config = TermConfig::default();
    // Override TERM for better compatibility
    term_config.env.insert("TERM".to_string(), "xterm-256color".to_string());
    tty::setup_env(&term_config);

    let mut settings = Settings::default();
    settings = settings.theme(config.app_theme.theme());

    #[cfg(target_os = "redox")]
    {
        // Redox does not support resize if doing CSDs
        settings = settings.client_decorations(false);
    }

    //TODO: allow size limits on iced_winit
    //settings = settings.size_limits(Limits::NONE.min_width(400.0).min_height(200.0));

    let flags = Flags {
        config_handler,
        config,
        term_config,
    };
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct Flags {
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    term_config: TermConfig,
}

#[derive(Clone, Debug)]
pub enum Action {
    Copy,
    Paste,
    SelectAll,
    Settings,
    TabNew,
}

impl Action {
    pub fn message(self, entity: segmented_button::Entity) -> Message {
        match self {
            Action::Copy => Message::Copy(Some(entity)),
            Action::Paste => Message::Paste(Some(entity)),
            Action::SelectAll => Message::SelectAll(Some(entity)),
            Action::Settings => Message::ToggleContextPage(ContextPage::Settings),
            Action::TabNew => Message::TabNew,
        }
    }
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    AppTheme(AppTheme),
    Config(Config),
    Copy(Option<segmented_button::Entity>),
    DefaultFont(usize),
    DefaultFontSize(usize),
    Paste(Option<segmented_button::Entity>),
    PasteValue(Option<segmented_button::Entity>, String),
    SelectAll(Option<segmented_button::Entity>),
    SystemThemeModeChange(cosmic_theme::ThemeMode),
    SyntaxTheme(usize, bool),
    TabActivate(segmented_button::Entity),
    TabClose(segmented_button::Entity),
    TabContextAction(segmented_button::Entity, Action),
    TabContextMenu(segmented_button::Entity, Option<Point>),
    TabNew,
    TermEvent(segmented_button::Entity, TermEvent),
    TermEventTx(mpsc::Sender<(segmented_button::Entity, TermEvent)>),
    ToggleContextPage(ContextPage),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPage {
    Settings,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            Self::Settings => fl!("settings"),
        }
    }
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    tab_model: segmented_button::Model<segmented_button::SingleSelect>,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    app_themes: Vec<String>,
    font_names: Vec<String>,
    font_size_names: Vec<String>,
    font_sizes: Vec<u16>,
    theme_names: Vec<String>,
    themes: HashMap<String, TermColors>,
    context_page: ContextPage,
    term_event_tx_opt: Option<mpsc::Sender<(segmented_button::Entity, TermEvent)>>,
    term_config: TermConfig,
}

impl App {
    fn update_config(&mut self) -> Command<Message> {
        //TODO: provide iterator over data
        let entities: Vec<_> = self.tab_model.iter().collect();
        for entity in entities {
            if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                let mut terminal = terminal.lock().unwrap();
                terminal.set_config(&self.config, &self.themes);
            }
        }
        cosmic::app::command::set_theme(self.config.app_theme.theme())
    }

    fn save_config(&mut self) -> Command<Message> {
        match self.config_handler {
            Some(ref config_handler) => match self.config.write_entry(&config_handler) {
                Ok(()) => {}
                Err(err) => {
                    log::error!("failed to save config: {}", err);
                }
            },
            None => {}
        }
        self.update_config()
    }

    fn update_title(&mut self) -> Command<Message> {
        let (header_title, window_title) = match self.tab_model.text(self.tab_model.active()) {
            Some(tab_title) => (
                tab_title.to_string(),
                format!("{tab_title} — COSMIC Terminal"),
            ),
            None => (String::new(), "COSMIC Terminal".to_string()),
        };
        self.set_header_title(header_title);
        self.set_window_title(window_title)
    }

    fn settings(&self) -> Element<Message> {
        let app_theme_selected = match self.config.app_theme {
            AppTheme::Dark => 1,
            AppTheme::Light => 2,
            AppTheme::System => 0,
        };
        let dark_selected = self
            .theme_names
            .iter()
            .position(|theme_name| theme_name == &self.config.syntax_theme_dark);
        let light_selected = self
            .theme_names
            .iter()
            .position(|theme_name| theme_name == &self.config.syntax_theme_light);
        let font_selected = {
            let mut font_system = font_system().write().unwrap();
            let current_font_name = font_system.raw().db().family_name(&Family::Monospace);
            self.font_names
                .iter()
                .position(|font_name| font_name == current_font_name)
        };
        let font_size_selected = self
            .font_sizes
            .iter()
            .position(|font_size| font_size == &self.config.font_size);
        widget::settings::view_column(vec![widget::settings::view_section(fl!("appearance"))
            .add(
                widget::settings::item::builder(fl!("theme")).control(widget::dropdown(
                    &self.app_themes,
                    Some(app_theme_selected),
                    move |index| {
                        Message::AppTheme(match index {
                            1 => AppTheme::Dark,
                            2 => AppTheme::Light,
                            _ => AppTheme::System,
                        })
                    },
                )),
            )
            .add(
                widget::settings::item::builder(fl!("syntax-dark")).control(widget::dropdown(
                    &self.theme_names,
                    dark_selected,
                    move |index| Message::SyntaxTheme(index, true),
                )),
            )
            .add(
                widget::settings::item::builder(fl!("syntax-light")).control(widget::dropdown(
                    &self.theme_names,
                    light_selected,
                    move |index| Message::SyntaxTheme(index, false),
                )),
            )
            .add(
                widget::settings::item::builder(fl!("default-font")).control(widget::dropdown(
                    &self.font_names,
                    font_selected,
                    |index| Message::DefaultFont(index),
                )),
            )
            .add(
                widget::settings::item::builder(fl!("default-font-size")).control(
                    widget::dropdown(&self.font_size_names, font_size_selected, |index| {
                        Message::DefaultFontSize(index)
                    }),
                ),
            )
            .into()])
        .into()
    }
}

/// Implement [`Application`] to integrate with COSMIC.
impl Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received
    type Flags = Flags;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicTerm";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(mut core: Core, flags: Self::Flags) -> (Self, Command<Self::Message>) {
        core.window.content_container = false;

        // Update font name from config
        {
            let mut font_system = font_system().write().unwrap();
            font_system
                .raw()
                .db_mut()
                .set_monospace_family(&flags.config.font_name);
        }

        let app_themes = vec![fl!("match-desktop"), fl!("dark"), fl!("light")];

        let font_names = {
            let mut font_names = Vec::new();
            let mut font_system = font_system().write().unwrap();
            //TODO: do not repeat, used in Tab::new
            let attrs = cosmic_text::Attrs::new().family(Family::Monospace);
            for face in font_system.raw().db().faces() {
                if attrs.matches(face) && face.monospaced {
                    //TODO: get localized name if possible
                    let font_name = face
                        .families
                        .get(0)
                        .map_or_else(|| face.post_script_name.to_string(), |x| x.0.to_string());
                    font_names.push(font_name);
                }
            }
            font_names.sort();
            font_names
        };

        let mut font_size_names = Vec::new();
        let mut font_sizes = Vec::new();
        for font_size in 4..=32 {
            font_size_names.push(format!("{}px", font_size));
            font_sizes.push(font_size);
        }

        let themes = terminal_theme::terminal_themes();
        let mut theme_names: Vec<_> = themes.keys().map(|x| x.clone()).collect();
        theme_names.sort();

        let mut app = App {
            core,
            tab_model: segmented_button::ModelBuilder::default().build(),
            config_handler: flags.config_handler,
            config: flags.config,
            app_themes,
            font_names,
            font_size_names,
            font_sizes,
            theme_names,
            themes,
            context_page: ContextPage::Settings,
            term_config: flags.term_config,
            term_event_tx_opt: None,
        };

        let command = app.update_title();

        (app, command)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::AppTheme(app_theme) => {
                self.config.app_theme = app_theme;
                return self.save_config();
            }
            Message::Config(config) => {
                if config != self.config {
                    log::info!("update config");
                    //TODO: update syntax theme by clearing tabs, only if needed
                    self.config = config;
                    return self.update_config();
                }
            }
            Message::Copy(entity_opt) => {
                let entity = entity_opt.unwrap_or_else(|| self.tab_model.active());
                if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                    let terminal = terminal.lock().unwrap();
                    let term = terminal.term.lock();
                    if let Some(text) = term.selection_to_string() {
                        return clipboard::write(text);
                    }
                }
            }
            Message::DefaultFont(index) => {
                match self.font_names.get(index) {
                    Some(font_name) => {
                        if font_name != &self.config.font_name {
                            // Update font name from config
                            {
                                let mut font_system = font_system().write().unwrap();
                                font_system.raw().db_mut().set_monospace_family(font_name);
                            }

                            let entities: Vec<_> = self.tab_model.iter().collect();
                            for entity in entities {
                                if let Some(terminal) =
                                    self.tab_model.data::<Mutex<Terminal>>(entity)
                                {
                                    let mut terminal = terminal.lock().unwrap();
                                    terminal.update_cell_size();
                                }
                            }

                            self.config.font_name = font_name.to_string();
                            return self.save_config();
                        }
                    }
                    None => {
                        log::warn!("failed to find font with index {}", index);
                    }
                }
            }
            Message::DefaultFontSize(index) => match self.font_sizes.get(index) {
                Some(font_size) => {
                    self.config.font_size = *font_size;
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find font with index {}", index);
                }
            },
            Message::Paste(entity_opt) => {
                return clipboard::read(move |value_opt| match value_opt {
                    Some(value) => message::app(Message::PasteValue(entity_opt, value)),
                    None => message::none(),
                });
            }
            Message::PasteValue(entity_opt, value) => {
                let entity = entity_opt.unwrap_or_else(|| self.tab_model.active());
                if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                    let terminal = terminal.lock().unwrap();
                    terminal.paste(value);
                }
            }
            Message::SelectAll(entity_opt) => {
                let entity = entity_opt.unwrap_or_else(|| self.tab_model.active());
                if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                    let mut terminal = terminal.lock().unwrap();
                    terminal.select_all();
                }
            }
            Message::SystemThemeModeChange(_theme_mode) => {
                return self.update_config();
            }
            Message::SyntaxTheme(index, dark) => match self.theme_names.get(index) {
                Some(theme_name) => {
                    if dark {
                        self.config.syntax_theme_dark = theme_name.to_string();
                    } else {
                        self.config.syntax_theme_light = theme_name.to_string();
                    }
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find syntax theme with index {}", index);
                }
            },
            Message::TabActivate(entity) => {
                self.tab_model.activate(entity);
                return self.update_title();
            }
            Message::TabClose(entity) => {
                // Activate closest item
                if let Some(position) = self.tab_model.position(entity) {
                    if position > 0 {
                        self.tab_model.activate_position(position - 1);
                    } else {
                        self.tab_model.activate_position(position + 1);
                    }
                }

                // Remove item
                self.tab_model.remove(entity);

                // If that was the last tab, close window
                if self.tab_model.iter().next().is_none() {
                    return window::close(window::Id::MAIN);
                }

                return self.update_title();
            }
            Message::TabContextAction(entity, action) => {
                match self.tab_model.data::<Mutex<Terminal>>(entity) {
                    Some(terminal) => {
                        // Close context menu
                        {
                            let mut terminal = terminal.lock().unwrap();
                            terminal.context_menu = None;
                        }
                        // Run action's message
                        return self.update(action.message(entity));
                    }
                    _ => {}
                }
            }
            Message::TabContextMenu(entity, position_opt) => {
                match self.tab_model.data::<Mutex<Terminal>>(entity) {
                    Some(terminal) => {
                        // Update context menu position
                        let mut terminal = terminal.lock().unwrap();
                        terminal.context_menu = position_opt;
                    }
                    _ => {}
                }
            }
            Message::TabNew => match &self.term_event_tx_opt {
                Some(term_event_tx) => match self.themes.get(self.config.syntax_theme()) {
                    Some(colors) => {
                        let entity = self
                            .tab_model
                            .insert()
                            .text("New Terminal")
                            .closable()
                            .activate()
                            .id();
                        let terminal = Terminal::new(
                            entity,
                            term_event_tx.clone(),
                            &self.term_config,
                            colors.clone(),
                        );
                        self.tab_model
                            .data_set::<Mutex<Terminal>>(entity, Mutex::new(terminal));
                    }
                    None => {
                        log::error!(
                            "failed to find terminal theme {:?}",
                            self.config.syntax_theme()
                        );
                        //TODO: fall back to known good theme
                    }
                },
                None => {
                    log::warn!("tried to create new tab before having event channel");
                }
            },
            Message::TermEvent(entity, event) => match event {
                TermEvent::Bell => {
                    //TODO: audible or visible bell options?
                }
                TermEvent::ColorRequest(index, f) => {
                    if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        let rgb = terminal.colors()[index].unwrap_or_default();
                        let text = f(rgb);
                        terminal.input_no_scroll(text.into_bytes());
                    }
                }
                TermEvent::Exit => {
                    return self.update(Message::TabClose(entity));
                }
                TermEvent::PtyWrite(text) => {
                    if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        terminal.input_no_scroll(text.into_bytes());
                    }
                }
                TermEvent::ResetTitle => {
                    self.tab_model.text_set(entity, "New Terminal");
                    return self.update_title();
                }
                TermEvent::TextAreaSizeRequest(f) => {
                    if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        let text = f(terminal.size().into());
                        terminal.input_no_scroll(text.into_bytes());
                    }
                }
                TermEvent::Title(title) => {
                    self.tab_model.text_set(entity, title);
                    return self.update_title();
                }
                TermEvent::MouseCursorDirty | TermEvent::Wakeup => {
                    if let Some(terminal) = self.tab_model.data::<Mutex<Terminal>>(entity) {
                        let mut terminal = terminal.lock().unwrap();
                        terminal.update();
                    }
                }
                _ => {
                    println!("TODO: {:?}", event);
                }
            },
            Message::TermEventTx(term_event_tx) => {
                self.term_event_tx_opt = Some(term_event_tx);
            }
            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
                self.set_context_title(context_page.title());
            }
        }

        Command::none()
    }

    fn context_drawer(&self) -> Option<Element<Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::Settings => self.settings(),
        })
    }

    fn header_start(&self) -> Vec<Element<Self::Message>> {
        let cosmic_theme::Spacing { space_xxs, .. } = self.core().system_theme().cosmic().spacing;

        vec![row![
            widget::button(widget::icon::from_name("list-add-symbolic").size(16).icon())
                .on_press(Message::TabNew)
                .padding(space_xxs)
                .style(style::Button::Icon)
        ]
        .align_items(Alignment::Center)
        .into()]
    }

    fn header_end(&self) -> Vec<Element<Self::Message>> {
        let cosmic_theme::Spacing { space_xxs, .. } = self.core().system_theme().cosmic().spacing;

        vec![row![widget::button(
            widget::icon::from_name("preferences-system-symbolic")
                .size(16)
                .icon()
        )
        .on_press(Message::ToggleContextPage(ContextPage::Settings))
        .padding(space_xxs)
        .style(style::Button::Icon)]
        .align_items(Alignment::Center)
        .into()]
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = self.core().system_theme().cosmic().spacing;

        let mut tab_column = widget::column::with_capacity(1);

        if self.tab_model.iter().count() > 1 {
            tab_column = tab_column.push(
                widget::container(
                    widget::view_switcher::horizontal(&self.tab_model)
                        .button_height(32)
                        .button_spacing(space_xxs)
                        .on_activate(Message::TabActivate)
                        .on_close(Message::TabClose),
                )
                .style(style::Container::Background)
                .width(Length::Fill),
            );
        }

        let entity = self.tab_model.active();
        match self.tab_model.data::<Mutex<Terminal>>(entity) {
            Some(terminal) => {
                let terminal_box = terminal_box(terminal).on_context_menu(move |position_opt| {
                    Message::TabContextMenu(entity, position_opt)
                });

                let context_menu = {
                    let terminal = terminal.lock().unwrap();
                    terminal.context_menu
                };

                let tab_element: Element<'_, Message> = match context_menu {
                    Some(position) => widget::popover(
                        terminal_box.context_menu(position),
                        menu::context_menu(entity),
                    )
                    .position(position)
                    .into(),
                    None => terminal_box.into(),
                };
                tab_column = tab_column.push(tab_element);
            }
            None => {
                //TODO
            }
        }

        let content: Element<_> = tab_column.into();

        // Uncomment to debug layout:
        //content.explain(cosmic::iced::Color::WHITE)
        content
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct TerminalEventSubscription;
        struct ThemeSubscription;

        Subscription::batch([
            event::listen_with(|event, _status| match event {
                Event::Keyboard(KeyEvent::KeyPressed {
                    key_code: KeyCode::A,
                    modifiers,
                }) => {
                    if modifiers == Modifiers::CTRL | Modifiers::SHIFT {
                        Some(Message::SelectAll(None))
                    } else {
                        None
                    }
                }
                Event::Keyboard(KeyEvent::KeyPressed {
                    key_code: KeyCode::C,
                    modifiers,
                }) => {
                    if modifiers == Modifiers::CTRL | Modifiers::SHIFT {
                        Some(Message::Copy(None))
                    } else {
                        None
                    }
                }
                Event::Keyboard(KeyEvent::KeyPressed {
                    key_code: KeyCode::T,
                    modifiers,
                }) => {
                    if modifiers == Modifiers::CTRL | Modifiers::SHIFT {
                        Some(Message::TabNew)
                    } else {
                        None
                    }
                }
                Event::Keyboard(KeyEvent::KeyPressed {
                    key_code: KeyCode::V,
                    modifiers,
                }) => {
                    if modifiers == Modifiers::CTRL | Modifiers::SHIFT {
                        Some(Message::Paste(None))
                    } else {
                        None
                    }
                }
                _ => None,
            }),
            subscription::channel(
                TypeId::of::<TerminalEventSubscription>(),
                100,
                |mut output| async move {
                    let (event_tx, mut event_rx) = mpsc::channel(100);
                    output.send(Message::TermEventTx(event_tx)).await.unwrap();

                    // Create first terminal tab
                    output.send(Message::TabNew).await.unwrap();

                    while let Some((entity, event)) = event_rx.recv().await {
                        output
                            .send(Message::TermEvent(entity, event))
                            .await
                            .unwrap();
                    }

                    panic!("terminal event channel closed");
                },
            ),
            cosmic_config::config_subscription(
                TypeId::of::<ConfigSubscription>(),
                Self::APP_ID.into(),
                CONFIG_VERSION,
            )
            .map(|(_, res)| match res {
                Ok(config) => Message::Config(config),
                Err((errs, config)) => {
                    log::info!("errors loading config: {:?}", errs);
                    Message::Config(config)
                }
            }),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::version(),
            )
            .map(|(_, u)| match u {
                Ok(t) => Message::SystemThemeModeChange(t),
                Err((errs, t)) => {
                    log::info!("errors loading theme mode: {:?}", errs);
                    Message::SystemThemeModeChange(t)
                }
            }),
        ])
    }
}
