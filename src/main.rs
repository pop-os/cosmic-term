// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use alacritty_terminal::{
    event::Event as TermEvent, term::color::Colors as TermColors, term::Config as TermConfig, tty,
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
        window, Alignment, Event, Length, Padding, Point,
    },
    style,
    widget::{self, button, container, pane_grid, segmented_button, PaneGrid},
    Application, ApplicationExt, Element,
};
use cosmic_text::{fontdb::FaceInfo, Family, Stretch, Weight};
use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet, HashMap},
    env, process,
    sync::Mutex,
    time::Duration,
};
use tokio::sync::mpsc;

use config::{AppTheme, Config, CONFIG_VERSION};
mod config;

use icon_cache::IconCache;
mod icon_cache;

use key_bind::{key_binds, KeyBind};
mod key_bind;

mod localize;

use menu::menu_bar;
mod menu;

use terminal::{Terminal, TerminalPaneGrid, TerminalScroll};
mod terminal;

use terminal_box::terminal_box;
mod terminal_box;

mod terminal_theme;

lazy_static::lazy_static! {
    static ref ICON_CACHE: Mutex<IconCache> = Mutex::new(IconCache::new());
}

pub fn icon_cache_get(name: &'static str, size: u16) -> widget::icon::Icon {
    let mut icon_cache = ICON_CACHE.lock().unwrap();
    icon_cache.get(name, size)
}

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

    let mut shell_program_opt = None;
    let mut shell_args = Vec::new();
    let mut parse_flags = true;
    for arg in env::args().skip(1) {
        if parse_flags {
            match arg.as_str() {
                // These flags indicate the end of parsing flags
                "-e" | "--command" | "--" => {
                    parse_flags = false;
                }
                _ => {
                    //TODO: should this throw an error?
                    log::warn!("ignored argument {:?}", arg);
                }
            }
        } else if shell_program_opt.is_none() {
            shell_program_opt = Some(arg);
        } else {
            shell_args.push(arg);
        }
    }

    let startup_options = if let Some(shell_program) = shell_program_opt {
        let mut options = tty::Options::default();
        options.shell = Some(tty::Shell::new(shell_program, shell_args));
        Some(options)
    } else {
        None
    };

    let term_config = TermConfig::default();
    // Set up environmental variables for terminal
    tty::setup_env();
    // Override TERM for better compatibility
    env::set_var("TERM", "xterm-256color");

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
        startup_options,
        term_config,
    };
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct Flags {
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    startup_options: Option<tty::Options>,
    term_config: TermConfig,
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Copy,
    Find,
    PaneFocusDown,
    PaneFocusLeft,
    PaneFocusRight,
    PaneFocusUp,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneToggleMaximized,
    Paste,
    SelectAll,
    Settings,
    ShowHeaderBar(bool),
    TabActivate0,
    TabActivate1,
    TabActivate2,
    TabActivate3,
    TabActivate4,
    TabActivate5,
    TabActivate6,
    TabActivate7,
    TabActivate8,
    TabClose,
    TabNew,
    TabNext,
    TabPrev,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl Action {
    pub fn message(self, entity: segmented_button::Entity) -> Message {
        match self {
            Action::Copy => Message::Copy(Some(entity)),
            Action::Find => Message::Find(true),
            Action::PaneFocusDown => Message::PaneFocusAdjacent(pane_grid::Direction::Down),
            Action::PaneFocusLeft => Message::PaneFocusAdjacent(pane_grid::Direction::Left),
            Action::PaneFocusRight => Message::PaneFocusAdjacent(pane_grid::Direction::Right),
            Action::PaneFocusUp => Message::PaneFocusAdjacent(pane_grid::Direction::Up),
            Action::PaneSplitHorizontal => Message::PaneSplit(pane_grid::Axis::Horizontal),
            Action::PaneSplitVertical => Message::PaneSplit(pane_grid::Axis::Vertical),
            Action::PaneToggleMaximized => Message::PaneToggleMaximized,
            Action::Paste => Message::Paste(Some(entity)),
            Action::SelectAll => Message::SelectAll(Some(entity)),
            Action::Settings => Message::ToggleContextPage(ContextPage::Settings),
            Action::ShowHeaderBar(show_headerbar) => Message::ShowHeaderBar(show_headerbar),
            Action::TabActivate0 => Message::TabActivateJump(0),
            Action::TabActivate1 => Message::TabActivateJump(1),
            Action::TabActivate2 => Message::TabActivateJump(2),
            Action::TabActivate3 => Message::TabActivateJump(3),
            Action::TabActivate4 => Message::TabActivateJump(4),
            Action::TabActivate5 => Message::TabActivateJump(5),
            Action::TabActivate6 => Message::TabActivateJump(6),
            Action::TabActivate7 => Message::TabActivateJump(7),
            Action::TabActivate8 => Message::TabActivateJump(8),
            Action::TabClose => Message::TabClose(None),
            Action::TabNew => Message::TabNew,
            Action::TabNext => Message::TabNext,
            Action::TabPrev => Message::TabPrev,
            Action::ZoomIn => Message::ZoomIn,
            Action::ZoomOut => Message::ZoomOut,
            Action::ZoomReset => Message::ZoomReset,
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
    DefaultFontStretch(usize),
    DefaultFontWeight(usize),
    DefaultDimFontWeight(usize),
    DefaultBoldFontWeight(usize),
    DefaultZoomStep(usize),
    Key(Modifiers, KeyCode),
    Find(bool),
    FindNext,
    FindPrevious,
    FindSearchValueChanged(String),
    PaneClicked(pane_grid::Pane),
    PaneSplit(pane_grid::Axis),
    PaneToggleMaximized,
    PaneFocusAdjacent(pane_grid::Direction),
    PaneDragged(pane_grid::DragEvent),
    PaneResized(pane_grid::ResizeEvent),
    Modifiers(Modifiers),
    Paste(Option<segmented_button::Entity>),
    PasteValue(Option<segmented_button::Entity>, String),
    SelectAll(Option<segmented_button::Entity>),
    UseBrightBold(bool),
    ShowHeaderBar(bool),
    SyntaxTheme(usize, bool),
    SystemThemeModeChange(cosmic_theme::ThemeMode),
    TabActivate(segmented_button::Entity),
    TabActivateJump(usize),
    TabClose(Option<segmented_button::Entity>),
    TabContextAction(segmented_button::Entity, Action),
    TabContextMenu(segmented_button::Entity, Option<Point>),
    TabNew,
    TabPrev,
    TabNext,
    TermEvent(pane_grid::Pane, segmented_button::Entity, TermEvent),
    TermEventTx(mpsc::Sender<(pane_grid::Pane, segmented_button::Entity, TermEvent)>),
    ToggleContextPage(ContextPage),
    ShowAdvancedFontSettings(bool),
    WindowClose,
    WindowNew,
    ZoomIn,
    ZoomOut,
    ZoomReset,
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
    pane_model: TerminalPaneGrid,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    key_binds: HashMap<KeyBind, Action>,
    app_themes: Vec<String>,
    font_names: Vec<String>,
    font_size_names: Vec<String>,
    font_sizes: Vec<u16>,
    font_name_faces_map: BTreeMap<String, Vec<FaceInfo>>,
    all_font_weights_vals_names_map: BTreeMap<u16, String>,
    all_font_stretches_vals_names_map: BTreeMap<Stretch, String>,
    curr_font_weight_names: Vec<String>,
    curr_font_weights: Vec<u16>,
    curr_font_stretch_names: Vec<String>,
    curr_font_stretches: Vec<Stretch>,
    zoom_adj: i8,
    zoom_step_names: Vec<String>,
    zoom_steps: Vec<u16>,
    theme_names: Vec<String>,
    themes: HashMap<String, TermColors>,
    context_page: ContextPage,
    terminal_ids: HashMap<pane_grid::Pane, widget::Id>,
    find: bool,
    find_search_id: widget::Id,
    find_search_value: String,
    term_event_tx_opt: Option<mpsc::Sender<(pane_grid::Pane, segmented_button::Entity, TermEvent)>>,
    startup_options: Option<tty::Options>,
    term_config: TermConfig,
    show_advanced_font_settings: bool,
    modifiers: Modifiers,
}

impl App {
    fn update_config(&mut self) -> Command<Message> {
        //TODO: provide iterator over data
        let panes: Vec<_> = self.pane_model.panes.iter().collect();
        for (_pane, tab_model) in panes {
            let entities: Vec<_> = tab_model.iter().collect();
            for entity in entities {
                if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                    let mut terminal = terminal.lock().unwrap();
                    terminal.set_config(&self.config, &self.themes, self.zoom_adj);
                }
            }
        }

        self.core.window.show_headerbar = self.config.show_headerbar;
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

    fn update_focus(&self) -> Command<Message> {
        if self.core.window.show_context {
            Command::none()
        } else if self.find {
            widget::text_input::focus(self.find_search_id.clone())
        } else if let Some(terminal_id) = self.terminal_ids.get(&self.pane_model.focus).cloned() {
            widget::text_input::focus(terminal_id)
        } else {
            Command::none()
        }
    }

    // Call this any time the tab changes
    fn update_title(&mut self, pane: Option<pane_grid::Pane>) -> Command<Message> {
        let pane = pane.unwrap_or(self.pane_model.focus);
        if let Some(tab_model) = self.pane_model.panes.get(pane) {
            let (header_title, window_title) = match tab_model.text(tab_model.active()) {
                Some(tab_title) => (
                    tab_title.to_string(),
                    format!("{tab_title} — COSMIC Terminal"),
                ),
                None => (String::new(), "COSMIC Terminal".to_string()),
            };
            self.set_header_title(header_title);
            Command::batch([self.set_window_title(window_title), self.update_focus()])
        } else {
            log::error!("Failed to get the specific pane");
            Command::batch([
                self.set_window_title("COSMIC Terminal".to_string()),
                self.update_focus(),
            ])
        }
    }

    fn set_curr_font_weights_and_stretches(&mut self) {
        // check if config font_name is available first, if not, set it to first name in list
        if !self.font_names.contains(&self.config.font_name) {
            log::error!("'{}' is not in the font list", self.config.font_name);
            log::error!("setting font name to '{}'", self.font_names[0]);
            let _ = self.update(Message::DefaultFont(0));
        }

        let curr_font_faces = &self.font_name_faces_map[&self.config.font_name];

        self.curr_font_stretches = curr_font_faces
            .iter()
            .map(|face| face.stretch)
            .collect::<BTreeSet<_>>() // remove duplicates and sort
            .into_iter()
            .collect();

        self.curr_font_stretch_names = self
            .curr_font_stretches
            .iter()
            .map(|stretch| &self.all_font_stretches_vals_names_map[stretch])
            .cloned()
            .collect::<Vec<_>>();

        if !self
            .curr_font_stretches
            .contains(&self.config.typed_font_stretch())
        {
            self.config.font_stretch = Stretch::Normal.to_number();
        }

        let curr_weights = |conf_stretch| {
            curr_font_faces
                .iter()
                .filter(|face| face.stretch == conf_stretch)
                .map(|face| face.weight.0)
                .collect::<BTreeSet<_>>() // remove duplicates and sort
                .into_iter()
                .collect()
        };

        self.curr_font_weights = curr_weights(self.config.typed_font_stretch());

        if self.curr_font_weights.is_empty() {
            // stretch fallback
            self.config.font_stretch = Stretch::Normal.to_number();
        }

        self.curr_font_weights = curr_weights(self.config.typed_font_stretch());
        assert!(!self.curr_font_weights.is_empty());

        self.curr_font_weight_names = self
            .curr_font_weights
            .iter()
            .map(|weight| &self.all_font_weights_vals_names_map[weight])
            .cloned()
            .collect::<Vec<_>>();

        if !self.curr_font_weights.contains(&self.config.font_weight) {
            self.config.font_weight = Weight::NORMAL.0;
        }

        if !self
            .curr_font_weights
            .contains(&self.config.dim_font_weight)
        {
            self.config.dim_font_weight = Weight::NORMAL.0;
        }

        if !self
            .curr_font_weights
            .contains(&self.config.bold_font_weight)
        {
            self.config.bold_font_weight = Weight::BOLD.0;
        }
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
        let font_stretch_selected = self
            .curr_font_stretches
            .iter()
            .position(|font_stretch| font_stretch == &self.config.typed_font_stretch());
        let font_weight_selected = self
            .curr_font_weights
            .iter()
            .position(|font_weight| font_weight == &self.config.font_weight);
        let dim_font_weight_selected = self
            .curr_font_weights
            .iter()
            .position(|font_weight| font_weight == &self.config.dim_font_weight);
        let bold_font_weight_selected = self
            .curr_font_weights
            .iter()
            .position(|font_weight| font_weight == &self.config.bold_font_weight);
        let zoom_step_selected = self
            .zoom_steps
            .iter()
            .position(|zoom_step| zoom_step == &self.config.font_size_zoom_step_mul_100);

        let advanced_font_settings = || {
            let section = widget::settings::view_section("")
                .add(
                    widget::settings::item::builder(fl!("default-font-stretch")).control(
                        widget::dropdown(
                            &self.curr_font_stretch_names,
                            font_stretch_selected,
                            |index| Message::DefaultFontStretch(index),
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("default-font-weight")).control(
                        widget::dropdown(
                            &self.curr_font_weight_names,
                            font_weight_selected,
                            |index| Message::DefaultFontWeight(index),
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("default-dim-font-weight")).control(
                        widget::dropdown(
                            &self.curr_font_weight_names,
                            dim_font_weight_selected,
                            |index| Message::DefaultDimFontWeight(index),
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("default-bold-font-weight")).control(
                        widget::dropdown(
                            &self.curr_font_weight_names,
                            bold_font_weight_selected,
                            |index| Message::DefaultBoldFontWeight(index),
                        ),
                    ),
                );
            let padding = Padding {
                top: 0.0,
                bottom: 0.0,
                left: 12.0,
                right: 12.0,
            };
            widget::container(section).padding(padding)
        };

        let mut settings_view = widget::settings::view_section(fl!("appearance"))
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
                widget::settings::item::builder(fl!("advanced-font-settings")).toggler(
                    self.show_advanced_font_settings,
                    Message::ShowAdvancedFontSettings,
                ),
            );

        if self.show_advanced_font_settings {
            settings_view = settings_view.add(advanced_font_settings());
        }

        let settings_view = settings_view
            .add(
                widget::settings::item::builder(fl!("use-bright-bold"))
                    .toggler(self.config.use_bright_bold, Message::UseBrightBold),
            )
            .add(
                widget::settings::item::builder(fl!("default-font-size")).control(
                    widget::dropdown(&self.font_size_names, font_size_selected, |index| {
                        Message::DefaultFontSize(index)
                    }),
                ),
            )
            .add(
                widget::settings::item::builder(fl!("default-zoom-step")).control(
                    widget::dropdown(&self.zoom_step_names, zoom_step_selected, |index| {
                        Message::DefaultZoomStep(index)
                    }),
                ),
            )
            .add(
                widget::settings::item::builder(fl!("show-headerbar"))
                    .toggler(self.config.show_headerbar, Message::ShowHeaderBar),
            );

        widget::settings::view_column(vec![settings_view.into()]).into()
    }

    fn create_and_focus_new_terminal(&mut self, pane: pane_grid::Pane) {
        self.pane_model.focus = pane;
        match &self.term_event_tx_opt {
            Some(term_event_tx) => match self.themes.get(self.config.syntax_theme()) {
                Some(colors) => {
                    let current_pane = self.pane_model.focus;
                    if let Some(tab_model) = self.pane_model.active_mut() {
                        let entity = tab_model
                            .insert()
                            .text("New Terminal")
                            .closable()
                            .activate()
                            .id();
                        // Use the startup options, or defaults
                        let options = self.startup_options.take().unwrap_or_default();
                        let mut terminal = Terminal::new(
                            current_pane,
                            entity,
                            term_event_tx.clone(),
                            self.term_config.clone(),
                            options,
                            &self.config,
                            *colors,
                        );
                        terminal.set_config(&self.config, &self.themes, self.zoom_adj);
                        tab_model.data_set::<Mutex<Terminal>>(entity, Mutex::new(terminal));
                    } else {
                        log::error!("Found no active pane");
                    }
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
        }
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
        //TODO: fix window resizing interfering with scrolling when not using content container
        //core.window.content_container = false;
        core.window.show_headerbar = flags.config.show_headerbar;

        // Update font name from config
        {
            let mut font_system = font_system().write().unwrap();
            font_system
                .raw()
                .db_mut()
                .set_monospace_family(&flags.config.font_name);
        }

        let app_themes = vec![fl!("match-desktop"), fl!("dark"), fl!("light")];

        let font_name_faces_map = {
            let mut font_name_faces_map = BTreeMap::<_, Vec<_>>::new();
            let mut font_system = font_system().write().unwrap();
            //TODO: do not repeat, used in Tab::new
            for face in font_system.raw().db().faces() {
                // only monospace fonts and weights that match named constants.
                let weight = face.weight.0;
                if face.monospaced && { 1..9 }.contains(&{ weight / 100 }) && weight % 100 == 0 {
                    //TODO: get localized name if possible
                    let font_name = face
                        .families
                        .get(0)
                        .map_or_else(|| face.post_script_name.to_string(), |x| x.0.to_string());
                    font_name_faces_map
                        .entry(font_name)
                        .or_default()
                        .push(face.clone());
                }
            }

            // only keep fonts that have both NORMAL and BOLD weights with both having
            // a `Stretch::Normal` face.
            // This is important for fallbacks.
            font_name_faces_map.retain(|_, v| {
                let has_normal = v
                    .iter()
                    .any(|face| face.weight == Weight::NORMAL && face.stretch == Stretch::Normal);
                let has_bold = v
                    .iter()
                    .any(|face| face.weight == Weight::BOLD && face.stretch == Stretch::Normal);
                has_normal && has_bold
            });
            font_name_faces_map
        };
        let font_names = font_name_faces_map.keys().cloned().collect();

        let mut font_size_names = Vec::new();
        let mut font_sizes = Vec::new();
        for font_size in 4..=32 {
            font_size_names.push(format!("{}px", font_size));
            font_sizes.push(font_size);
        }

        let mut all_font_weights_vals_names_map = BTreeMap::new();

        macro_rules! populate_font_weights {
            ($($weight:ident,)+) => {
                // all weights
                paste::paste!{
                    $(
                        all_font_weights_vals_names_map
                            .insert(Weight::$weight.0, stringify!([<$weight:camel>]).into());
                    )+
                }
            };
        }

        populate_font_weights! {
            THIN, EXTRA_LIGHT, LIGHT, NORMAL, MEDIUM,
            SEMIBOLD, BOLD, EXTRA_BOLD, BLACK,
        };

        let mut all_font_stretches_vals_names_map = BTreeMap::new();

        macro_rules! populate_font_stretches {
            ($($stretch:ident,)+) => {
                // all stretches
                $(
                    all_font_stretches_vals_names_map
                        .insert(Stretch::$stretch, stringify!($stretch).into());
                )+
            };
        }

        populate_font_stretches! {
            UltraCondensed, ExtraCondensed, Condensed, SemiCondensed,
            Normal, SemiExpanded, Expanded, ExtraExpanded, UltraExpanded,
        };

        let mut zoom_step_names = Vec::new();
        let mut zoom_steps = Vec::new();
        for zoom_step in [25, 50, 75, 100, 150, 200] {
            zoom_step_names.push(format!("{}px", f32::from(zoom_step) / 100.0));
            zoom_steps.push(zoom_step);
        }

        let themes = terminal_theme::terminal_themes();
        let mut theme_names: Vec<_> = themes.keys().map(|x| x.clone()).collect();
        theme_names.sort();
        let pane_model = TerminalPaneGrid::new(segmented_button::ModelBuilder::default().build());
        let mut terminal_ids = HashMap::new();
        terminal_ids.insert(pane_model.focus, widget::Id::unique());

        let mut app = App {
            core,
            pane_model,
            config_handler: flags.config_handler,
            config: flags.config,
            key_binds: key_binds(),
            app_themes,
            font_names,
            font_size_names,
            font_sizes,
            font_name_faces_map,
            all_font_weights_vals_names_map,
            all_font_stretches_vals_names_map,
            curr_font_weight_names: Vec::new(),
            curr_font_weights: Vec::new(),
            curr_font_stretch_names: Vec::new(),
            curr_font_stretches: Vec::new(),
            zoom_adj: 0,
            zoom_step_names,
            zoom_steps,
            theme_names,
            themes,
            context_page: ContextPage::Settings,
            terminal_ids,
            find: false,
            find_search_id: widget::Id::unique(),
            find_search_value: String::new(),
            startup_options: flags.startup_options,
            term_config: flags.term_config,
            term_event_tx_opt: None,
            show_advanced_font_settings: false,
            modifiers: Modifiers::empty(),
        };

        app.set_curr_font_weights_and_stretches();
        let command = app.update_title(None);

        (app, command)
    }

    //TODO: currently the first escape unfocuses, and the second calls this function
    fn on_escape(&mut self) -> Command<Message> {
        if self.core.window.show_context {
            // Close context drawer if open
            self.core.window.show_context = false;
        } else if self.find {
            // Close find if open
            self.find = false;
        }

        // Focus correct widget
        self.update_focus()
    }

    fn on_context_drawer(&mut self) -> Command<Message> {
        if !self.core.window.show_context {
            self.update_focus()
        } else {
            Command::none()
        }
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
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        let term = terminal.term.lock();
                        if let Some(text) = term.selection_to_string() {
                            return clipboard::write(text);
                        }
                    }
                } else {
                    log::warn!("Failed to get focused pane");
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
                            let panes: Vec<_> = self.pane_model.panes.iter().collect();
                            for (_pane, tab_model) in panes {
                                let entities: Vec<_> = tab_model.iter().collect();
                                for entity in entities {
                                    if let Some(terminal) =
                                        tab_model.data::<Mutex<Terminal>>(entity)
                                    {
                                        let mut terminal = terminal.lock().unwrap();
                                        terminal.update_cell_size();
                                    }
                                }
                            }

                            self.config.font_name = font_name.to_string();
                            self.set_curr_font_weights_and_stretches();

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
                    self.zoom_adj = 0; // reset zoom
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find font with index {}", index);
                }
            },
            Message::DefaultFontStretch(index) => match self.curr_font_stretches.get(index) {
                Some(font_stretch) => {
                    self.config.font_stretch = font_stretch.to_number();
                    self.set_curr_font_weights_and_stretches();
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find font weight with index {}", index);
                }
            },
            Message::DefaultFontWeight(index) => match self.curr_font_weights.get(index) {
                Some(font_weight) => {
                    self.config.font_weight = *font_weight;
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find font weight with index {}", index);
                }
            },
            Message::DefaultDimFontWeight(index) => match self.curr_font_weights.get(index) {
                Some(font_weight) => {
                    self.config.dim_font_weight = *font_weight;
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find dim font weight with index {}", index);
                }
            },
            Message::DefaultBoldFontWeight(index) => match self.curr_font_weights.get(index) {
                Some(font_weight) => {
                    self.config.bold_font_weight = *font_weight;
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find bold font weight with index {}", index);
                }
            },
            Message::DefaultZoomStep(index) => match self.zoom_steps.get(index) {
                Some(zoom_step) => {
                    self.config.font_size_zoom_step_mul_100 = *zoom_step;
                    self.zoom_adj = 0; // reset zoom
                    return self.save_config();
                }
                None => {
                    log::warn!("failed to find zoom step with index {}", index);
                }
            },
            Message::Key(modifiers, key_code) => {
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = tab_model.active();
                    for (key_bind, action) in self.key_binds.iter() {
                        if key_bind.matches(modifiers, key_code) {
                            return self.update(action.message(entity));
                        }
                    }
                }
            }
            Message::Find(find) => {
                self.find = find;

                // Focus correct input
                return self.update_focus();
            }
            Message::FindNext => {
                if !self.find_search_value.is_empty() {
                    if let Some(tab_model) = self.pane_model.active() {
                        let entity = tab_model.active();
                        if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                            let mut terminal = terminal.lock().unwrap();
                            terminal.search(&self.find_search_value, true);
                        }
                    }
                }

                // Focus correct input
                return self.update_focus();
            }
            Message::FindPrevious => {
                if !self.find_search_value.is_empty() {
                    if let Some(tab_model) = self.pane_model.active() {
                        let entity = tab_model.active();
                        if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                            let mut terminal = terminal.lock().unwrap();
                            terminal.search(&self.find_search_value, false);
                        }
                    }
                }

                // Focus correct input
                return self.update_focus();
            }
            Message::FindSearchValueChanged(value) => {
                self.find_search_value = value;
            }
            Message::Modifiers(modifiers) => {
                self.modifiers = modifiers;
            }
            Message::PaneClicked(pane) => {
                self.pane_model.focus = pane;
                return self.update_title(Some(pane));
            }
            Message::PaneSplit(axis) => {
                let result = self.pane_model.panes.split(
                    axis,
                    self.pane_model.focus,
                    segmented_button::ModelBuilder::default().build(),
                );
                if let Some((pane, _)) = result {
                    self.terminal_ids.insert(pane, widget::Id::unique());
                    self.create_and_focus_new_terminal(pane);
                    self.pane_model.panes_created += 1;
                    return self.update_title(Some(pane));
                }
            }
            Message::PaneToggleMaximized => {
                if self.pane_model.panes.maximized().is_some() {
                    self.pane_model.panes.restore();
                } else {
                    self.pane_model.panes.maximize(self.pane_model.focus);
                }
                return self.update_focus();
            }
            Message::PaneFocusAdjacent(direction) => {
                if let Some(adjacent) = self
                    .pane_model
                    .panes
                    .adjacent(self.pane_model.focus, direction)
                {
                    self.pane_model.focus = adjacent;
                    return self.update_title(Some(adjacent));
                }
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.pane_model.panes.resize(split, ratio);
            }
            Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.pane_model.panes.drop(pane, target);
            }
            Message::PaneDragged(_) => {}
            Message::Paste(entity_opt) => {
                return clipboard::read(move |value_opt| match value_opt {
                    Some(value) => message::app(Message::PasteValue(entity_opt, value)),
                    None => message::none(),
                });
            }
            Message::PasteValue(entity_opt, value) => {
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        terminal.paste(value);
                    }
                }
            }
            Message::SelectAll(entity_opt) => {
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let mut terminal = terminal.lock().unwrap();
                        terminal.select_all();
                    }
                }
            }
            Message::ShowHeaderBar(show_headerbar) => {
                if show_headerbar != self.config.show_headerbar {
                    self.config.show_headerbar = show_headerbar;
                    return self.save_config();
                }
            }
            Message::UseBrightBold(use_bright_bold) => {
                if use_bright_bold != self.config.use_bright_bold {
                    self.config.use_bright_bold = use_bright_bold;
                    return self.save_config();
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
                if let Some(tab_model) = self.pane_model.active_mut() {
                    tab_model.activate(entity);
                }
                return self.update_title(None);
            }
            Message::TabActivateJump(pos) => {
                if let Some(tab_model) = self.pane_model.active() {
                    // Length is always at least one so there shouldn't be a division by zero
                    let len = tab_model.iter().count();
                    // The typical pattern is that 1-8 selects tabs 1-8 while 9 selects the last tab
                    let pos = if pos >= 8 || pos > len - 1 {
                        len - 1
                    } else {
                        pos % len
                    };

                    let entity = tab_model.iter().nth(pos);
                    if let Some(entity) = entity {
                        return self.update(Message::TabActivate(entity));
                    }
                }
            }
            Message::TabClose(entity_opt) => {
                if let Some(tab_model) = self.pane_model.active_mut() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());

                    // Activate closest item
                    if let Some(position) = tab_model.position(entity) {
                        if position > 0 {
                            tab_model.activate_position(position - 1);
                        } else {
                            tab_model.activate_position(position + 1);
                        }
                    }

                    // Remove item
                    tab_model.remove(entity);

                    // If that was the last tab, close current pane
                    if tab_model.iter().next().is_none() {
                        if let Some((_state, sibling)) =
                            self.pane_model.panes.close(self.pane_model.focus)
                        {
                            self.terminal_ids.remove(&self.pane_model.focus);
                            self.pane_model.focus = sibling;
                        } else {
                            //Last pane, closing window
                            return window::close(window::Id::MAIN);
                        }
                    }
                }

                return self.update_title(None);
            }
            Message::TabContextAction(entity, action) => {
                if let Some(tab_model) = self.pane_model.active() {
                    match tab_model.data::<Mutex<Terminal>>(entity) {
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
            }
            Message::TabContextMenu(entity, position_opt) => {
                if let Some(tab_model) = self.pane_model.active() {
                    match tab_model.data::<Mutex<Terminal>>(entity) {
                        Some(terminal) => {
                            // Update context menu position
                            let mut terminal = terminal.lock().unwrap();
                            terminal.context_menu = position_opt;
                        }
                        _ => {}
                    }
                }
            }
            Message::TabNew => self.create_and_focus_new_terminal(self.pane_model.focus),
            Message::TabNext => {
                if let Some(tab_model) = self.pane_model.active() {
                    let len = tab_model.iter().count();
                    // Next tab position. Wraps around to 0 (first tab) if the last tab is active.
                    let pos = tab_model
                        .position(tab_model.active())
                        .map(|i| (i as usize + 1) % len)
                        .expect("at least one tab is always open");

                    let entity = tab_model.iter().nth(pos);
                    if let Some(entity) = entity {
                        return self.update(Message::TabActivate(entity));
                    }
                }
            }
            Message::TabPrev => {
                if let Some(tab_model) = self.pane_model.active() {
                    let pos = tab_model
                        .position(tab_model.active())
                        .and_then(|i| (i as usize).checked_sub(1))
                        .unwrap_or_else(|| {
                            tab_model.iter().count().checked_sub(1).unwrap_or_default()
                        });

                    let entity = tab_model.iter().nth(pos);
                    if let Some(entity) = entity {
                        return self.update(Message::TabActivate(entity));
                    }
                }
            }
            Message::TermEvent(pane, entity, event) => {
                match event {
                    TermEvent::Bell => {
                        //TODO: audible or visible bell options?
                    }
                    TermEvent::ColorRequest(index, f) => {
                        if let Some(tab_model) = self.pane_model.panes.get(pane) {
                            if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                                let terminal = terminal.lock().unwrap();
                                let rgb = terminal.colors()[index].unwrap_or_default();
                                let text = f(rgb);
                                terminal.input_no_scroll(text.into_bytes());
                            }
                        }
                    }
                    TermEvent::Exit => {
                        return self.update(Message::TabClose(Some(entity)));
                    }
                    TermEvent::PtyWrite(text) => {
                        if let Some(tab_model) = self.pane_model.panes.get(pane) {
                            if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                                let terminal = terminal.lock().unwrap();
                                terminal.input_no_scroll(text.into_bytes());
                            }
                        }
                    }
                    TermEvent::ResetTitle => {
                        if let Some(tab_model) = self.pane_model.panes.get_mut(pane) {
                            tab_model.text_set(entity, "New Terminal");
                        }
                        return self.update_title(Some(pane));
                    }
                    TermEvent::TextAreaSizeRequest(f) => {
                        if let Some(tab_model) = self.pane_model.panes.get(pane) {
                            if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                                let terminal = terminal.lock().unwrap();
                                let text = f(terminal.size().into());
                                terminal.input_no_scroll(text.into_bytes());
                            }
                        }
                    }
                    TermEvent::Title(title) => {
                        if let Some(tab_model) = self.pane_model.panes.get_mut(pane) {
                            tab_model.text_set(entity, title);
                        }
                        return self.update_title(Some(pane));
                    }
                    TermEvent::MouseCursorDirty | TermEvent::Wakeup => {
                        if let Some(tab_model) = self.pane_model.panes.get(pane) {
                            if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                                let mut terminal = terminal.lock().unwrap();
                                terminal.needs_update = true;
                            }
                        }
                    }
                    _ => {
                        log::warn!("TODO: {:?}", event);
                    }
                }
            }
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
            Message::ShowAdvancedFontSettings(show) => {
                self.show_advanced_font_settings = show;
            }
            Message::WindowClose => {
                return window::close(window::Id::MAIN);
            }
            Message::WindowNew => match env::current_exe() {
                Ok(exe) => match process::Command::new(&exe).spawn() {
                    Ok(_child) => {}
                    Err(err) => {
                        log::error!("failed to execute {:?}: {}", exe, err);
                    }
                },
                Err(err) => {
                    log::error!("failed to get current executable path: {}", err);
                }
            },
            Message::ZoomIn => {
                self.zoom_adj = self.zoom_adj.saturating_add(1);
                return self.save_config();
            }
            Message::ZoomOut => {
                self.zoom_adj = self.zoom_adj.saturating_sub(1);
                return self.save_config();
            }
            Message::ZoomReset => {
                self.zoom_adj = 0;
                return self.save_config();
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
        vec![menu_bar().into()]
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = self.core().system_theme().cosmic().spacing;
        let pane_grid = PaneGrid::new(&self.pane_model.panes, |pane, tab_model, _is_maximized| {
            let mut tab_column = widget::column::with_capacity(1);

            if tab_model.iter().count() > 1 {
                tab_column = tab_column.push(
                    widget::container(
                        widget::view_switcher::horizontal(tab_model)
                            .button_height(32)
                            .button_spacing(space_xxs)
                            .on_activate(Message::TabActivate)
                            .on_close(|entity| Message::TabClose(Some(entity))),
                    )
                    .style(style::Container::Background)
                    .width(Length::Fill),
                );
            }

            let entity = tab_model.active();
            let terminal_id = self
                .terminal_ids
                .get(&pane)
                .cloned()
                .unwrap_or_else(widget::Id::unique);
            match tab_model.data::<Mutex<Terminal>>(entity) {
                Some(terminal) => {
                    let terminal_box = terminal_box(terminal).id(terminal_id).on_context_menu(
                        move |position_opt| Message::TabContextMenu(entity, position_opt),
                    );

                    let context_menu = {
                        let terminal = terminal.lock().unwrap();
                        terminal.context_menu
                    };

                    let tab_element: Element<'_, Message> = match context_menu {
                        Some(position) => widget::popover(
                            terminal_box.context_menu(position),
                            menu::context_menu(&self.config, entity),
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

            if self.find {
                let find_input = widget::text_input::text_input(
                    fl!("find-placeholder"),
                    &self.find_search_value,
                )
                .id(self.find_search_id.clone())
                .on_input(Message::FindSearchValueChanged)
                // This is inverted for ease of use, usually in terminals you want to search
                // upwards, which is FindPrevious
                .on_submit(if self.modifiers.contains(Modifiers::SHIFT) {
                    Message::FindNext
                } else {
                    Message::FindPrevious
                })
                .width(Length::Fixed(320.0))
                .trailing_icon(
                    button(icon_cache_get("edit-clear-symbolic", 16))
                        .on_press(Message::FindSearchValueChanged(String::new()))
                        .style(style::Button::Icon)
                        .into(),
                );
                let find_widget = widget::row::with_children(vec![
                    find_input.into(),
                    widget::tooltip(
                        button(icon_cache_get("go-up-symbolic", 16))
                            .on_press(Message::FindPrevious)
                            .padding(space_xxs)
                            .style(style::Button::Icon),
                        fl!("find-previous"),
                        widget::tooltip::Position::Top,
                    )
                    .into(),
                    widget::tooltip(
                        button(icon_cache_get("go-down-symbolic", 16))
                            .on_press(Message::FindNext)
                            .padding(space_xxs)
                            .style(style::Button::Icon),
                        fl!("find-next"),
                        widget::tooltip::Position::Top,
                    )
                    .into(),
                    widget::horizontal_space(Length::Fill).into(),
                    button(icon_cache_get("window-close-symbolic", 16))
                        .on_press(Message::Find(false))
                        .padding(space_xxs)
                        .style(style::Button::Icon)
                        .into(),
                ])
                .align_items(Alignment::Center)
                .padding(space_xxs)
                .spacing(space_xxs);

                tab_column = tab_column.push(
                    widget::cosmic_container::container(find_widget)
                        .layer(cosmic_theme::Layer::Primary),
                );
            }

            pane_grid::Content::new(tab_column)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(space_xxs)
        .on_click(Message::PaneClicked)
        .on_resize(space_xxs, Message::PaneResized)
        .on_drag(Message::PaneDragged);

        container(pane_grid)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(space_xxs)
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct TerminalEventSubscription;
        struct ThemeSubscription;

        Subscription::batch([
            event::listen_with(|event, _status| match event {
                Event::Keyboard(KeyEvent::KeyPressed {
                    key_code,
                    modifiers,
                }) => Some(Message::Key(modifiers, key_code)),
                Event::Keyboard(KeyEvent::ModifiersChanged(modifiers)) => {
                    Some(Message::Modifiers(modifiers))
                }
                _ => None,
            }),
            subscription::channel(
                TypeId::of::<TerminalEventSubscription>(),
                100,
                |mut output| async move {
                    let (event_tx, mut event_rx) = mpsc::channel(100);
                    output.send(Message::TermEventTx(event_tx)).await.unwrap();

                    // Avoid creating two tabs at startup
                    tokio::time::sleep(Duration::from_millis(50)).await;

                    // Create first terminal tab
                    output.send(Message::TabNew).await.unwrap();

                    while let Some((pane, entity, event)) = event_rx.recv().await {
                        output
                            .send(Message::TermEvent(pane, entity, event))
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
            .map(|update| {
                if !update.errors.is_empty() {
                    log::debug!(
                        "errors loading config {:?}: {:?}",
                        update.keys,
                        update.errors
                    );
                }
                Message::Config(update.config)
            }),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::version(),
            )
            .map(|update| {
                if !update.errors.is_empty() {
                    log::debug!(
                        "errors loading theme mode {:?}: {:?}",
                        update.keys,
                        update.errors
                    );
                }
                Message::SystemThemeModeChange(update.config)
            }),
        ])
    }
}
