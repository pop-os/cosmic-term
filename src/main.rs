// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use alacritty_terminal::{event::Event as TermEvent, term, term::color::Colors as TermColors, tty};
use cosmic::iced::clipboard::dnd::DndAction;
use cosmic::widget::menu::action::MenuAction;
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::widget::DndDestination;
use cosmic::Apply;
use cosmic::{
    app::{command, message, Core, Settings, Task},
    cosmic_config::{self, ConfigSet, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        self,
        advanced::graphics::text::font_system,
        clipboard, event,
        futures::SinkExt,
        keyboard::{Event as KeyEvent, Key, Modifiers},
        mouse::{Button as MouseButton, Event as MouseEvent},
        stream, window, Alignment, Color, Event, Length, Limits, Padding, Point, Subscription,
    },
    style,
    widget::{self, button, pane_grid, segmented_button, PaneGrid},
    Application, ApplicationExt, Element,
};
use cosmic_files::dialog::{Dialog, DialogKind, DialogMessage, DialogResult};
use cosmic_text::{fontdb::FaceInfo, Family, Stretch, Weight};
use localize::LANGUAGE_SORTER;
use std::{
    any::TypeId,
    cmp,
    collections::{BTreeMap, BTreeSet, HashMap},
    env, fs, process,
    rc::Rc,
    sync::{atomic::Ordering, Mutex},
};
use tokio::sync::mpsc;

use config::{
    AppTheme, ColorScheme, ColorSchemeId, ColorSchemeKind, Config, Profile, ProfileId,
    CONFIG_VERSION,
};
mod config;
mod mouse_reporter;

use icon_cache::IconCache;
mod icon_cache;

use key_bind::key_binds;
mod key_bind;

mod localize;

use menu::menu_bar;
mod menu;

use terminal::{Terminal, TerminalPaneGrid, TerminalScroll};
mod terminal;

use terminal_box::terminal_box;

use crate::dnd::DndDrop;
mod terminal_box;

mod terminal_theme;

mod dnd;

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
    let mut shell_program_opt = None;
    let mut shell_args = Vec::new();
    let mut parse_flags = true;
    let mut daemonize = true;
    for arg in env::args().skip(1) {
        if parse_flags {
            match arg.as_str() {
                // These flags indicate the end of parsing flags
                "-e" | "--command" | "--" => {
                    parse_flags = false;
                }
                "--no-daemon" => {
                    daemonize = false;
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

    #[cfg(all(unix, not(target_os = "redox")))]
    if daemonize {
        match fork::daemon(true, true) {
            Ok(fork::Fork::Child) => (),
            Ok(fork::Fork::Parent(_child_pid)) => process::exit(0),
            Err(err) => {
                eprintln!("failed to daemonize: {:?}", err);
                process::exit(1);
            }
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

    let startup_options = if let Some(shell_program) = shell_program_opt {
        let options = tty::Options {
            shell: Some(tty::Shell::new(shell_program, shell_args)),
            ..tty::Options::default()
        };
        Some(options)
    } else {
        None
    };

    let term_config = term::Config::default();
    // Set up environmental variables for terminal
    tty::setup_env();
    // Override TERM for better compatibility
    env::set_var("TERM", "xterm-256color");

    let mut settings = Settings::default();
    settings = settings.theme(config.app_theme.theme());
    settings = settings.size_limits(Limits::NONE.min_width(360.0).min_height(180.0));

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
    term_config: term::Config,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    About,
    ClearScrollback,
    ColorSchemes(ColorSchemeKind),
    Copy,
    CopyOrSigint,
    CopyPrimary,
    Find,
    PaneFocusDown,
    PaneFocusLeft,
    PaneFocusRight,
    PaneFocusUp,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneToggleMaximized,
    Paste,
    PastePrimary,
    ProfileOpen(ProfileId),
    Profiles,
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
    TabNewNoProfile,
    TabNext,
    TabPrev,
    WindowClose,
    WindowNew,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

impl Action {
    fn message(&self, entity_opt: Option<segmented_button::Entity>) -> Message {
        match self {
            Self::About => Message::ToggleContextPage(ContextPage::About),
            Self::ClearScrollback => Message::ClearScrollback(entity_opt),
            Self::ColorSchemes(color_scheme_kind) => {
                Message::ToggleContextPage(ContextPage::ColorSchemes(*color_scheme_kind))
            }
            Self::Copy => Message::Copy(entity_opt),
            Self::CopyOrSigint => Message::CopyOrSigint(entity_opt),
            Self::CopyPrimary => Message::CopyPrimary(entity_opt),
            Self::Find => Message::Find(true),
            Self::PaneFocusDown => Message::PaneFocusAdjacent(pane_grid::Direction::Down),
            Self::PaneFocusLeft => Message::PaneFocusAdjacent(pane_grid::Direction::Left),
            Self::PaneFocusRight => Message::PaneFocusAdjacent(pane_grid::Direction::Right),
            Self::PaneFocusUp => Message::PaneFocusAdjacent(pane_grid::Direction::Up),
            Self::PaneSplitHorizontal => Message::PaneSplit(pane_grid::Axis::Horizontal),
            Self::PaneSplitVertical => Message::PaneSplit(pane_grid::Axis::Vertical),
            Self::PaneToggleMaximized => Message::PaneToggleMaximized,
            Self::Paste => Message::Paste(entity_opt),
            Self::PastePrimary => Message::PastePrimary(entity_opt),
            Self::ProfileOpen(profile_id) => Message::ProfileOpen(*profile_id),
            Self::Profiles => Message::ToggleContextPage(ContextPage::Profiles),
            Self::SelectAll => Message::SelectAll(entity_opt),
            Self::Settings => Message::ToggleContextPage(ContextPage::Settings),
            Self::ShowHeaderBar(show_headerbar) => Message::ShowHeaderBar(*show_headerbar),
            Self::TabActivate0 => Message::TabActivateJump(0),
            Self::TabActivate1 => Message::TabActivateJump(1),
            Self::TabActivate2 => Message::TabActivateJump(2),
            Self::TabActivate3 => Message::TabActivateJump(3),
            Self::TabActivate4 => Message::TabActivateJump(4),
            Self::TabActivate5 => Message::TabActivateJump(5),
            Self::TabActivate6 => Message::TabActivateJump(6),
            Self::TabActivate7 => Message::TabActivateJump(7),
            Self::TabActivate8 => Message::TabActivateJump(8),
            Self::TabClose => Message::TabClose(entity_opt),
            Self::TabNew => Message::TabNew,
            Self::TabNewNoProfile => Message::TabNewNoProfile,
            Self::TabNext => Message::TabNext,
            Self::TabPrev => Message::TabPrev,
            Self::WindowClose => Message::WindowClose,
            Self::WindowNew => Message::WindowNew,
            Self::ZoomIn => Message::ZoomIn,
            Self::ZoomOut => Message::ZoomOut,
            Self::ZoomReset => Message::ZoomReset,
        }
    }
}

impl MenuAction for Action {
    type Message = Message;

    fn message(&self) -> Message {
        self.message(None)
    }
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    AppTheme(AppTheme),
    ClearScrollback(Option<segmented_button::Entity>),
    ColorSchemeCollapse,
    ColorSchemeDelete(ColorSchemeKind, ColorSchemeId),
    ColorSchemeExpand(ColorSchemeKind, Option<ColorSchemeId>),
    ColorSchemeExport(ColorSchemeKind, Option<ColorSchemeId>),
    ColorSchemeExportResult(ColorSchemeKind, Option<ColorSchemeId>, DialogResult),
    ColorSchemeImport(ColorSchemeKind),
    ColorSchemeImportResult(ColorSchemeKind, DialogResult),
    ColorSchemeRename(ColorSchemeKind, ColorSchemeId, String),
    ColorSchemeRenameSubmit,
    ColorSchemeTabActivate(widget::segmented_button::Entity),
    Config(Config),
    Copy(Option<segmented_button::Entity>),
    CopyOrSigint(Option<segmented_button::Entity>),
    CopyPrimary(Option<segmented_button::Entity>),
    DefaultBoldFontWeight(usize),
    DefaultDimFontWeight(usize),
    DefaultFont(usize),
    DefaultFontSize(usize),
    DefaultFontStretch(usize),
    DefaultFontWeight(usize),
    DefaultZoomStep(usize),
    DialogMessage(DialogMessage),
    Drop(Option<(pane_grid::Pane, segmented_button::Entity, DndDrop)>),
    Find(bool),
    FindNext,
    FindPrevious,
    FindSearchValueChanged(String),
    MiddleClick(pane_grid::Pane, Option<segmented_button::Entity>),
    FocusFollowMouse(bool),
    Key(Modifiers, Key),
    LaunchUrl(String),
    Modifiers(Modifiers),
    MouseEnter(pane_grid::Pane),
    Opacity(u8),
    PaneClicked(pane_grid::Pane),
    PaneDragged(pane_grid::DragEvent),
    PaneFocusAdjacent(pane_grid::Direction),
    PaneResized(pane_grid::ResizeEvent),
    PaneSplit(pane_grid::Axis),
    PaneToggleMaximized,
    Paste(Option<segmented_button::Entity>),
    PastePrimary(Option<segmented_button::Entity>),
    PasteValue(Option<segmented_button::Entity>, String),
    ProfileCollapse(ProfileId),
    ProfileCommand(ProfileId, String),
    ProfileDirectory(ProfileId, String),
    ProfileExpand(ProfileId),
    ProfileHold(ProfileId, bool),
    ProfileName(ProfileId, String),
    ProfileNew,
    ProfileOpen(ProfileId),
    ProfileRemove(ProfileId),
    ProfileSyntaxTheme(ProfileId, ColorSchemeKind, usize),
    ProfileTabTitle(ProfileId, String),
    SelectAll(Option<segmented_button::Entity>),
    ShowAdvancedFontSettings(bool),
    ShowHeaderBar(bool),
    SyntaxTheme(ColorSchemeKind, usize),
    SystemThemeChange,
    TabActivate(segmented_button::Entity),
    TabActivateJump(usize),
    TabClose(Option<segmented_button::Entity>),
    TabContextAction(segmented_button::Entity, Action),
    TabContextMenu(pane_grid::Pane, Option<Point>),
    TabNew,
    TabNewNoProfile,
    TabNext,
    TabPrev,
    TermEvent(pane_grid::Pane, segmented_button::Entity, TermEvent),
    TermEventTx(mpsc::UnboundedSender<(pane_grid::Pane, segmented_button::Entity, TermEvent)>),
    ToggleContextPage(ContextPage),
    UpdateDefaultProfile((bool, ProfileId)),
    UseBrightBold(bool),
    WindowClose,
    WindowNew,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPage {
    About,
    ColorSchemes(ColorSchemeKind),
    Profiles,
    Settings,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            Self::About => String::new(),
            Self::ColorSchemes(_color_scheme_kind) => fl!("color-schemes"),
            Self::Profiles => fl!("profiles"),
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
    zoom_step_names: Vec<String>,
    zoom_steps: Vec<u16>,
    theme_names_dark: Vec<String>,
    theme_names_light: Vec<String>,
    themes: HashMap<(String, ColorSchemeKind), TermColors>,
    context_page: ContextPage,
    dialog_opt: Option<Dialog<Message>>,
    terminal_ids: HashMap<pane_grid::Pane, widget::Id>,
    find: bool,
    find_search_id: widget::Id,
    find_search_value: String,
    term_event_tx_opt:
        Option<mpsc::UnboundedSender<(pane_grid::Pane, segmented_button::Entity, TermEvent)>>,
    startup_options: Option<tty::Options>,
    term_config: term::Config,
    color_scheme_errors: Vec<String>,
    color_scheme_expanded: Option<(ColorSchemeKind, Option<ColorSchemeId>)>,
    color_scheme_renaming: Option<(ColorSchemeKind, ColorSchemeId, String)>,
    color_scheme_rename_id: widget::Id,
    color_scheme_tab_model: widget::segmented_button::SingleSelectModel,
    profile_expanded: Option<ProfileId>,
    show_advanced_font_settings: bool,
    modifiers: Modifiers,
}

impl App {
    fn theme_names(&self, color_scheme_kind: ColorSchemeKind) -> &Vec<String> {
        match color_scheme_kind {
            ColorSchemeKind::Dark => &self.theme_names_dark,
            ColorSchemeKind::Light => &self.theme_names_light,
        }
    }

    fn update_color_schemes(&mut self) {
        self.themes = terminal_theme::terminal_themes();
        for &color_scheme_kind in &[ColorSchemeKind::Dark, ColorSchemeKind::Light] {
            for (color_scheme_name, color_scheme_id) in
                self.config.color_scheme_names(color_scheme_kind)
            {
                if let Some(color_scheme) = self
                    .config
                    .color_schemes(color_scheme_kind)
                    .get(&color_scheme_id)
                {
                    if self
                        .themes
                        .insert(
                            (color_scheme_name.clone(), color_scheme_kind),
                            color_scheme.into(),
                        )
                        .is_some()
                    {
                        log::warn!(
                            "custom {:?} color scheme {:?} replaces builtin one",
                            color_scheme_kind,
                            color_scheme_name
                        );
                    }
                }
            }
        }

        self.theme_names_dark.clear();
        self.theme_names_light.clear();
        for (name, color_scheme_kind) in self.themes.keys() {
            match *color_scheme_kind {
                ColorSchemeKind::Dark => {
                    self.theme_names_dark.push(name.clone());
                }
                ColorSchemeKind::Light => {
                    self.theme_names_light.push(name.clone());
                }
            }
        }
        self.theme_names_dark
            .sort_by(|a, b| LANGUAGE_SORTER.compare(a, b));
        self.theme_names_light
            .sort_by(|a, b| LANGUAGE_SORTER.compare(a, b));
    }

    fn reset_terminal_panes_zoom(&mut self) {
        for (_pane, tab_model) in self.pane_model.panes.iter() {
            for entity in tab_model.iter() {
                if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                    let mut terminal = terminal.lock().unwrap();
                    terminal.set_zoom_adj(0);
                }
            }
        }
    }

    fn update_config(&mut self) -> Task<Message> {
        let theme = self.config.app_theme.theme();

        // Update color schemes
        self.update_color_schemes();

        // Update terminal window background color
        {
            let color = Color::from(theme.cosmic().background.base);
            let bytes = color.into_rgba8();
            let data = u32::from(bytes[2])
                | (u32::from(bytes[1]) << 8)
                | (u32::from(bytes[0]) << 16)
                | 0xFF000000;
            terminal::WINDOW_BG_COLOR.store(data, Ordering::SeqCst);
        }

        // Set config of all tabs
        for (_pane, tab_model) in self.pane_model.panes.iter() {
            for entity in tab_model.iter() {
                if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                    let mut terminal = terminal.lock().unwrap();
                    terminal.set_config(&self.config, &self.themes);
                }
            }
        }

        // Set headerbar state
        self.core.window.show_headerbar = self.config.show_headerbar;

        // Update application theme
        cosmic::app::command::set_theme(theme)
    }

    fn update_render_active_pane_zoom(&mut self, zoom_message: Message) -> Task<Message> {
        // skip writing config to fs when zoom in/ out
        // recalculate the pane due to the changes of zoom_adj value
        // but only for the active pane/tab
        if let Some(tab_model) = self.pane_model.active() {
            for entity in tab_model.iter() {
                if tab_model.is_active(entity) {
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let mut terminal = terminal.lock().unwrap();
                        let current_zoom_adj = terminal.zoom_adj();
                        match zoom_message {
                            Message::ZoomIn => {
                                terminal.set_zoom_adj(current_zoom_adj.saturating_add(1))
                            }
                            Message::ZoomOut => {
                                terminal.set_zoom_adj(current_zoom_adj.saturating_sub(1))
                            }
                            _ => {}
                        }
                        terminal.set_config(&self.config, &self.themes);
                    }
                }
            }
        }
        Task::none()
    }

    fn save_color_schemes(&mut self, color_scheme_kind: ColorSchemeKind) -> Task<Message> {
        // Optimized for just saving color_schemes
        if let Some(ref config_handler) = self.config_handler {
            if let Err(err) = config_handler.set(
                match color_scheme_kind {
                    ColorSchemeKind::Dark => "color_schemes_dark",
                    ColorSchemeKind::Light => "color_schemes_light",
                },
                self.config.color_schemes(color_scheme_kind),
            ) {
                log::error!("failed to save config: {}", err);
            }
        }
        self.update_color_schemes();
        Task::none()
    }

    fn save_profiles(&mut self) -> Task<Message> {
        // Optimized for just saving profiles
        if let Some(ref config_handler) = self.config_handler {
            match config_handler.set("profiles", &self.config.profiles) {
                Ok(()) => {}
                Err(err) => {
                    log::error!("failed to save config: {}", err);
                }
            }
        }
        Task::none()
    }

    fn update_focus(&self) -> Task<Message> {
        if self.find {
            widget::text_input::focus(self.find_search_id.clone())
        } else if let Some(terminal_id) = self.terminal_ids.get(&self.pane_model.focus).cloned() {
            widget::text_input::focus(terminal_id)
        } else {
            Task::none()
        }
    }

    // Call this any time the tab changes
    fn update_title(&mut self, pane: Option<pane_grid::Pane>) -> Task<Message> {
        let pane = pane.unwrap_or(self.pane_model.focus);
        if let Some(tab_model) = self.pane_model.panes.get(pane) {
            let (header_title, window_title) = match tab_model.text(tab_model.active()) {
                Some(tab_title) => (
                    tab_title.to_string(),
                    format!("{tab_title} — {}", fl!("cosmic-terminal")),
                ),
                None => (String::new(), fl!("cosmic-terminal")),
            };
            self.set_header_title(header_title);
            Task::batch([
                if let Some(window_id) = self.core.main_window_id() {
                    self.set_window_title(window_title, window_id)
                } else {
                    Task::none()
                },
                self.update_focus(),
            ])
        } else {
            log::error!("Failed to get the specific pane");
            Task::batch([
                if let Some(window_id) = self.core.main_window_id() {
                    self.set_window_title(fl!("cosmic-terminal"), window_id)
                } else {
                    Task::none()
                },
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

    fn about(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = self.core().system_theme().cosmic().spacing;
        let repository = "https://github.com/pop-os/cosmic-term";
        let hash = env!("VERGEN_GIT_SHA");
        let short_hash: String = hash.chars().take(7).collect();
        let date = env!("VERGEN_GIT_COMMIT_DATE");
        widget::column::with_children(vec![
                widget::svg(widget::svg::Handle::from_memory(
                    &include_bytes!(
                        "../res/icons/hicolor/128x128/apps/com.system76.CosmicTerm.svg"
                    )[..],
                ))
                .into(),
                widget::text::title3(fl!("cosmic-terminal")).into(),
                widget::button::link(repository)
                    .on_press(Message::LaunchUrl(repository.to_string()))
                    .padding(0)
                    .into(),
                widget::button::link(fl!(
                    "git-description",
                    hash = short_hash.as_str(),
                    date = date
                ))
                    .on_press(Message::LaunchUrl(format!("{repository}/commits/{hash}")))
                    .padding(0)
                .into(),
            ])
        .align_x(Alignment::Center)
        .spacing(space_xxs)
        .into()
    }

    fn color_schemes(&self, color_scheme_kind: ColorSchemeKind) -> Element<Message> {
        let cosmic_theme::Spacing { space_xxxs, .. } = self.core().system_theme().cosmic().spacing;

        let mut sections = Vec::with_capacity(3 + self.color_scheme_errors.len());

        sections.push(
            widget::tab_bar::horizontal(&self.color_scheme_tab_model)
                .on_activate(Message::ColorSchemeTabActivate)
                .into(),
        );

        let mut section = widget::settings::section();
        let builtin_name = format!("COSMIC {:?}", color_scheme_kind);
        let color_scheme_names = self.config.color_scheme_names(color_scheme_kind);
        for (color_scheme_name, color_scheme_id_opt) in std::iter::once((builtin_name, None)).chain(
            color_scheme_names
                .into_iter()
                .map(|(name, id)| (name, Some(id))),
        ) {
            let expanded =
                self.color_scheme_expanded == Some((color_scheme_kind, color_scheme_id_opt));
            let renaming = match &self.color_scheme_renaming {
                Some((kind, id, value))
                    if kind == &color_scheme_kind && Some(id) == color_scheme_id_opt.as_ref() =>
                {
                    Some(value)
                }
                _ => None,
            };

            let button = if expanded {
                widget::button::custom(icon_cache_get("view-more-symbolic", 16))
                    .on_press(Message::ColorSchemeCollapse)
            } else {
                widget::button::custom(icon_cache_get("view-more-symbolic", 16)).on_press(
                    Message::ColorSchemeExpand(color_scheme_kind, color_scheme_id_opt),
                )
            }
            .class(style::Button::Icon);

            let mut popover = widget::popover(button);
            if expanded {
                let menu = menu::color_scheme_menu(
                    color_scheme_kind,
                    color_scheme_id_opt,
                    &color_scheme_name,
                );
                popover = popover
                    .popup(menu)
                    .position(widget::popover::Position::Bottom);
            }

            let item = match renaming {
                Some(value) => widget::settings::item_row(vec![
                    widget::text_input("", value)
                        .id(self.color_scheme_rename_id.clone())
                        .on_input(move |value| {
                            Message::ColorSchemeRename(
                                color_scheme_kind,
                                color_scheme_id_opt.expect("trying to rename builtin color scheme"),
                                value,
                            )
                        })
                        .on_submit(Message::ColorSchemeRenameSubmit)
                        .into(),
                    popover.into(),
                ]),
                None => widget::settings::item::builder(color_scheme_name).control(popover),
            };
            section = section.add(item);
        }
        sections.push(section.into());

        sections.push(
            widget::row::with_children(vec![
                widget::horizontal_space().into(),
                widget::button::standard(fl!("import"))
                    .on_press(Message::ColorSchemeImport(color_scheme_kind))
                    .into(),
            ])
            .into(),
        );

        for error in &self.color_scheme_errors {
            sections.push(
                widget::row::with_children(vec![
                    icon_cache_get("dialog-error-symbolic", 16)
                        .class(style::Svg::Custom(Rc::new(|theme| {
                            let cosmic = theme.cosmic();
                            widget::svg::Style {
                                color: Some(cosmic.destructive_text_color().into()),
                            }
                        })))
                        .into(),
                    widget::text::body(error)
                        .class(style::Text::Custom(|theme| {
                            let cosmic = theme.cosmic();
                            //TODO: re-export in libcosmic
                            iced::widget::text::Style {
                                color: Some(cosmic.destructive_text_color().into()),
                            }
                        }))
                        .into(),
                ])
                .spacing(space_xxxs)
                .into(),
            );
        }

        widget::settings::view_column(sections).into()
    }

    fn profiles(&self) -> Element<Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_xs,
            space_xxs,
            space_xxxs,
            ..
        } = self.core().system_theme().cosmic().spacing;

        let mut sections = Vec::with_capacity(2);

        if !self.config.profiles.is_empty() {
            let mut profiles_section = widget::settings::section();
            for (profile_name, profile_id) in self.config.profile_names() {
                let Some(profile) = self.config.profiles.get(&profile_id) else {
                    continue;
                };

                let expanded = self.profile_expanded == Some(profile_id);

                profiles_section = profiles_section.add(
                    widget::settings::item::builder(profile_name).control(
                        widget::row::with_children(vec![
                            widget::button::custom(icon_cache_get("edit-delete-symbolic", 16))
                                .on_press(Message::ProfileRemove(profile_id))
                                .class(style::Button::Icon)
                                .into(),
                            if expanded {
                                widget::button::custom(icon_cache_get("go-up-symbolic", 16))
                                    .on_press(Message::ProfileCollapse(profile_id))
                            } else {
                                widget::button::custom(icon_cache_get("go-down-symbolic", 16))
                                    .on_press(Message::ProfileExpand(profile_id))
                            }
                            .class(style::Button::Icon)
                            .into(),
                        ])
                        .align_y(Alignment::Center)
                        .spacing(space_xxs),
                    ),
                );

                if expanded {
                    let dark_selected = self
                        .theme_names_dark
                        .iter()
                        .position(|theme_name| theme_name == &profile.syntax_theme_dark);
                    let light_selected = self
                        .theme_names_light
                        .iter()
                        .position(|theme_name| theme_name == &profile.syntax_theme_light);

                    let expanded_section = widget::settings::section()
                        .add(
                            widget::column::with_children(vec![
                                widget::column::with_children(vec![
                                    widget::text(fl!("name")).into(),
                                    widget::text_input("", &profile.name)
                                        .on_input(move |text| {
                                            Message::ProfileName(profile_id, text)
                                        })
                                        .into(),
                                ])
                                .spacing(space_xxxs)
                                .into(),
                                widget::column::with_children(vec![
                                    widget::text(fl!("command-line")).into(),
                                    widget::text_input("", &profile.command)
                                        .on_input(move |text| {
                                            Message::ProfileCommand(profile_id, text)
                                        })
                                        .into(),
                                ])
                                .spacing(space_xxxs)
                                .into(),
                                widget::column::with_children(vec![
                                    widget::text(fl!("working-directory")).into(),
                                    widget::text_input("", &profile.working_directory)
                                        .on_input(move |text| {
                                            Message::ProfileDirectory(profile_id, text)
                                        })
                                        .into(),
                                ])
                                .spacing(space_xxxs)
                                .into(),
                                widget::column::with_children(vec![
                                    widget::text(fl!("tab-title")).into(),
                                    widget::text_input("", &profile.tab_title)
                                        .on_input(move |text| {
                                            Message::ProfileTabTitle(profile_id, text)
                                        })
                                        .into(),
                                    widget::text::caption(fl!("tab-title-description")).into(),
                                ])
                                .spacing(space_xxxs)
                                .into(),
                            ])
                            .padding([0, space_s])
                            .spacing(space_xs),
                        )
                        .add(
                            //TODO: rename to color-scheme-dark?
                            widget::settings::item::builder(fl!("syntax-dark")).control(
                                widget::dropdown(
                                    &self.theme_names_dark,
                                    dark_selected,
                                    move |theme_i| {
                                        Message::ProfileSyntaxTheme(
                                            profile_id,
                                            ColorSchemeKind::Dark,
                                            theme_i,
                                        )
                                    },
                                ),
                            ),
                        )
                        .add(
                            //TODO: rename to color-scheme-light?
                            widget::settings::item::builder(fl!("syntax-light")).control(
                                widget::dropdown(
                                    &self.theme_names_light,
                                    light_selected,
                                    move |theme_i| {
                                        Message::ProfileSyntaxTheme(
                                            profile_id,
                                            ColorSchemeKind::Light,
                                            theme_i,
                                        )
                                    },
                                ),
                            ),
                        )
                        .add(
                            widget::settings::item::builder(fl!("make-default")).control(
                                widget::toggler(
                                    self.get_default_profile().is_some_and(|p| p == profile_id),
                                )
                                .on_toggle(move |t| Message::UpdateDefaultProfile((t, profile_id))),
                            ),
                        )
                        .add(
                            widget::row::with_children(vec![
                                widget::column::with_children(vec![
                                    widget::text(fl!("hold")).into(),
                                    widget::text::caption(fl!("remain-open")).into(),
                                ])
                                .spacing(space_xxxs)
                                .into(),
                                widget::horizontal_space().into(),
                                widget::toggler(profile.hold)
                                    .on_toggle(move |t| Message::ProfileHold(profile_id, t))
                                    .into(),
                            ])
                            .align_y(Alignment::Center)
                            .padding([0, space_s]),
                        );

                    let padding = Padding {
                        top: 0.0,
                        bottom: 0.0,
                        left: space_s.into(),
                        right: space_s.into(),
                    };
                    profiles_section =
                        profiles_section.add(widget::container(expanded_section).padding(padding))
                }
            }
            sections.push(profiles_section.into());
        }

        let add_profile = widget::row::with_children(vec![
            widget::horizontal_space().into(),
            widget::button::standard(fl!("add-profile"))
                .on_press(Message::ProfileNew)
                .into(),
        ]);
        sections.push(add_profile.into());

        widget::settings::view_column(sections).into()
    }

    fn settings(&self) -> Element<Message> {
        let app_theme_selected = match self.config.app_theme {
            AppTheme::Dark => 1,
            AppTheme::Light => 2,
            AppTheme::System => 0,
        };
        let dark_selected = self
            .theme_names_dark
            .iter()
            .position(|theme_name| theme_name == &self.config.syntax_theme_dark);
        let light_selected = self
            .theme_names_light
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

        let appearance_section = widget::settings::section()
            .title(fl!("appearance"))
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
                //TODO: rename to color-scheme-dark?
                widget::settings::item::builder(fl!("syntax-dark")).control(widget::dropdown(
                    &self.theme_names_dark,
                    dark_selected,
                    move |index| Message::SyntaxTheme(ColorSchemeKind::Dark, index),
                )),
            )
            .add(
                //TODO: rename to color-scheme-light?
                widget::settings::item::builder(fl!("syntax-light")).control(widget::dropdown(
                    &self.theme_names_light,
                    light_selected,
                    move |index| Message::SyntaxTheme(ColorSchemeKind::Light, index),
                )),
            )
            .add(
                widget::settings::item::builder(fl!("default-zoom-step")).control(
                    widget::dropdown(&self.zoom_step_names, zoom_step_selected, |index| {
                        Message::DefaultZoomStep(index)
                    }),
                ),
            )
            .add(
                widget::settings::item::builder(fl!("opacity"))
                    .description(format!("{}%", self.config.opacity))
                    .control(widget::slider(0..=100, self.config.opacity, |opacity| {
                        Message::Opacity(opacity)
                    })),
            );

        let mut font_section = widget::settings::section()
            .title(fl!("font"))
            .add(
                widget::settings::item::builder(fl!("default-font")).control(widget::dropdown(
                    &self.font_names,
                    font_selected,
                    Message::DefaultFont,
                )),
            )
            .add(
                widget::settings::item::builder(fl!("default-font-size")).control(
                    widget::dropdown(&self.font_size_names, font_size_selected, |index| {
                        Message::DefaultFontSize(index)
                    }),
                ),
            )
            .add(
                widget::settings::item::builder(fl!("advanced-font-settings")).control(
                    if self.show_advanced_font_settings {
                        widget::button::custom(icon_cache_get("go-up-symbolic", 16))
                            .on_press(Message::ShowAdvancedFontSettings(false))
                    } else {
                        widget::button::custom(icon_cache_get("go-down-symbolic", 16))
                            .on_press(Message::ShowAdvancedFontSettings(true))
                    }
                    .class(style::Button::Icon),
                ),
            );

        let advanced_font_settings = || {
            let section = widget::settings::section()
                .add(
                    widget::settings::item::builder(fl!("default-font-stretch")).control(
                        widget::dropdown(
                            &self.curr_font_stretch_names,
                            font_stretch_selected,
                            Message::DefaultFontStretch,
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("default-font-weight")).control(
                        widget::dropdown(
                            &self.curr_font_weight_names,
                            font_weight_selected,
                            Message::DefaultFontWeight,
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("default-dim-font-weight")).control(
                        widget::dropdown(
                            &self.curr_font_weight_names,
                            dim_font_weight_selected,
                            Message::DefaultDimFontWeight,
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("default-bold-font-weight")).control(
                        widget::dropdown(
                            &self.curr_font_weight_names,
                            bold_font_weight_selected,
                            Message::DefaultBoldFontWeight,
                        ),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("use-bright-bold"))
                        .toggler(self.config.use_bright_bold, Message::UseBrightBold),
                );
            let padding = Padding {
                top: 0.0,
                bottom: 0.0,
                left: 12.0,
                right: 12.0,
            };
            widget::container(section).padding(padding)
        };

        if self.show_advanced_font_settings {
            font_section = font_section.add(advanced_font_settings());
        }

        let splits_section = widget::settings::section().title(fl!("splits")).add(
            widget::settings::item::builder(fl!("focus-follow-mouse"))
                .toggler(self.config.focus_follow_mouse, Message::FocusFollowMouse),
        );

        let advanced_section = widget::settings::section().title(fl!("advanced")).add(
            widget::settings::item::builder(fl!("show-headerbar"))
                .description(fl!("show-header-description"))
                .toggler(self.config.show_headerbar, Message::ShowHeaderBar),
        );

        widget::settings::view_column(vec![
            appearance_section.into(),
            font_section.into(),
            splits_section.into(),
            advanced_section.into(),
        ])
        .into()
    }
    fn get_default_profile(&self) -> Option<ProfileId> {
        self.config.default_profile
    }

    fn create_and_focus_new_terminal(
        &mut self,
        pane: pane_grid::Pane,
        profile_id_opt: Option<ProfileId>,
    ) -> Task<Message> {
        self.pane_model.focus = pane;
        match &self.term_event_tx_opt {
            Some(term_event_tx) => {
                let colors = self
                    .themes
                    .get(&self.config.syntax_theme(profile_id_opt))
                    .or_else(|| match self.config.color_scheme_kind() {
                        ColorSchemeKind::Dark => self
                            .themes
                            .get(&(config::COSMIC_THEME_DARK.to_string(), ColorSchemeKind::Dark)),
                        ColorSchemeKind::Light => self.themes.get(&(
                            config::COSMIC_THEME_LIGHT.to_string(),
                            ColorSchemeKind::Light,
                        )),
                    });
                match colors {
                    Some(colors) => {
                        let current_pane = self.pane_model.focus;
                        if let Some(tab_model) = self.pane_model.active_mut() {
                            // Use the profile options, startup options, or defaults
                            let (options, tab_title_override) = match profile_id_opt
                                .and_then(|profile_id| self.config.profiles.get(&profile_id))
                            {
                                Some(profile) => {
                                    let mut shell = None;
                                    if let Some(mut args) = shlex::split(&profile.command) {
                                        if !args.is_empty() {
                                            let command = args.remove(0);
                                            shell = Some(tty::Shell::new(command, args));
                                        }
                                    }
                                    let working_directory = (!profile.working_directory.is_empty())
                                        .then(|| profile.working_directory.clone().into());

                                    let options = tty::Options {
                                        shell,
                                        working_directory,
                                        hold: profile.hold,
                                        env: HashMap::new(),
                                    };
                                    let tab_title_override = if profile.tab_title.is_empty() {
                                        None
                                    } else {
                                        Some(profile.tab_title.clone())
                                    };
                                    (options, tab_title_override)
                                }
                                None => (self.startup_options.take().unwrap_or_default(), None),
                            };
                            let entity = tab_model
                                .insert()
                                .text(
                                    tab_title_override
                                        .clone()
                                        .unwrap_or_else(|| fl!("new-terminal")),
                                )
                                .closable()
                                .activate()
                                .id();
                            match Terminal::new(
                                current_pane,
                                entity,
                                term_event_tx.clone(),
                                self.term_config.clone(),
                                options,
                                &self.config,
                                *colors,
                                profile_id_opt,
                                tab_title_override,
                            ) {
                                Ok(mut terminal) => {
                                    terminal.set_config(&self.config, &self.themes);
                                    tab_model
                                        .data_set::<Mutex<Terminal>>(entity, Mutex::new(terminal));
                                }
                                Err(err) if profile_id_opt.is_some() => {
                                    // Create a tab without a profile if the selected
                                    // profile doesn't work
                                    let name = profile_id_opt
                                        .and_then(|id| self.config.profiles.get(&id))
                                        .map(|profile| profile.name.as_str())
                                        .unwrap_or_default();
                                    log::error!(
                                        "failed to open terminal with profile `{}`: {}",
                                        name,
                                        err
                                    );

                                    // TabClose focuses the nearest tab which would be incorrect
                                    // in this specific case as it would unfocus the new tab
                                    // created by TabNewNoProfile
                                    // TabClose can also cause the terminal app to close if it
                                    // closes the only open tab. This would close cosmic term
                                    // if launched with an invalid profile (issue #274)
                                    tab_model.remove(entity);
                                    return self.update(Message::TabNewNoProfile);
                                }
                                Err(err) => {
                                    log::error!("failed to open terminal: {}", err);
                                    // Clean up partially created tab
                                    return self.update(Message::TabClose(Some(entity)));
                                }
                            }
                        } else {
                            log::error!("Found no active pane");
                        }
                    }
                    None => {
                        log::error!(
                            "failed to find terminal theme {:?}",
                            self.config.syntax_theme(profile_id_opt)
                        );
                        //TODO: fall back to known good theme
                    }
                }
            }
            None => {
                log::warn!("tried to create new tab before having event channel");
            }
        }
        self.update_title(Some(pane))
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
    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        core.window.content_container = false;
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
                        .first()
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

        if font_name_faces_map.is_empty() {
            log::error!("at least one monospace font with normal/bold weights and default stretch is required");
            log::error!("no monospace fonts to select from, exiting");
            process::exit(1);
        }

        let font_names = font_name_faces_map.keys().cloned().collect();

        let mut font_size_names = Vec::new();
        let mut font_sizes = Vec::new();
        for font_size in 4..=32 {
            font_size_names.push(format!("{font_size}px"));
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

        let pane_model = TerminalPaneGrid::new(segmented_button::ModelBuilder::default().build());
        let mut terminal_ids = HashMap::new();
        terminal_ids.insert(pane_model.focus, widget::Id::unique());

        let mut app = Self {
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
            zoom_step_names,
            zoom_steps,
            theme_names_dark: Vec::new(),
            theme_names_light: Vec::new(),
            themes: HashMap::new(),
            context_page: ContextPage::Settings,
            dialog_opt: None,
            terminal_ids,
            find: false,
            find_search_id: widget::Id::unique(),
            find_search_value: String::new(),
            startup_options: flags.startup_options,
            term_config: flags.term_config,
            term_event_tx_opt: None,
            color_scheme_errors: Vec::new(),
            color_scheme_expanded: None,
            color_scheme_renaming: None,
            color_scheme_rename_id: widget::Id::unique(),
            color_scheme_tab_model: widget::segmented_button::Model::default(),
            profile_expanded: None,
            show_advanced_font_settings: false,
            modifiers: Modifiers::empty(),
        };

        app.set_curr_font_weights_and_stretches();
        let command = Task::batch([app.update_config(), app.update_title(None)]);

        (app, command)
    }

    //TODO: currently the first escape unfocuses, and the second calls this function
    fn on_escape(&mut self) -> Task<Message> {
        if self.core.window.show_context {
            // Close context drawer if open
            self.core.window.show_context = false;
        } else if self.find {
            // Close find if open
            self.find = false;
            self.find_search_value.clear();
        }

        // Focus correct widget
        self.update_focus()
    }

    fn on_context_drawer(&mut self) -> Task<Message> {
        if self.core.window.show_context {
            Task::none()
        } else {
            self.update_focus()
        }
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        // Helper for updating config values efficiently
        macro_rules! config_set {
            ($name: ident, $value: expr) => {
                match &self.config_handler {
                    Some(config_handler) => {
                        if let Err(err) =
                            paste::paste! { self.config.[<set_ $name>](config_handler, $value) }
                        {
                            log::warn!("failed to save config {:?}: {}", stringify!($name), err);
                        }
                    }
                    None => {
                        self.config.$name = $value;
                        log::warn!(
                            "failed to save config {:?}: no config handler",
                            stringify!($name)
                        );
                    }
                }
            };
        }
        match message {
            Message::AppTheme(app_theme) => {
                config_set!(app_theme, app_theme);
                return self.update_config();
            }
            Message::ClearScrollback(entity_opt) => {
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        let mut term = terminal.term.lock();
                        term.grid_mut().clear_history();
                    }
                }
            }
            Message::ColorSchemeCollapse => {
                self.color_scheme_expanded = None;
            }
            Message::ColorSchemeDelete(color_scheme_kind, color_scheme_id) => {
                self.color_scheme_expanded = None;
                self.config
                    .color_schemes_mut(color_scheme_kind)
                    .remove(&color_scheme_id);
                return self.save_color_schemes(color_scheme_kind);
            }
            Message::ColorSchemeExport(color_scheme_kind, color_scheme_id_opt) => {
                self.color_scheme_expanded = None;
                if let Some(color_scheme_name) = match color_scheme_id_opt {
                    Some(color_scheme_id) => self
                        .config
                        .color_schemes(color_scheme_kind)
                        .get(&color_scheme_id)
                        .map(|color_scheme| color_scheme.name.clone()),
                    None => Some(format!("COSMIC {:?}", color_scheme_kind)),
                } {
                    if self.dialog_opt.is_none() {
                        let (dialog, command) = Dialog::new(
                            DialogKind::SaveFile {
                                filename: format!("{}.ron", color_scheme_name),
                            },
                            None,
                            Message::DialogMessage,
                            move |result| {
                                Message::ColorSchemeExportResult(
                                    color_scheme_kind,
                                    color_scheme_id_opt,
                                    result,
                                )
                            },
                        );
                        self.dialog_opt = Some(dialog);
                        return command;
                    }
                }
            }
            Message::ColorSchemeExportResult(color_scheme_kind, color_scheme_id_opt, result) => {
                //TODO: show errors in UI
                self.dialog_opt = None;
                if let DialogResult::Open(paths) = result {
                    let path = &paths[0];
                    match color_scheme_id_opt {
                        Some(color_scheme_id) => {
                            if let Some(color_scheme) = self
                                .config
                                .color_schemes(color_scheme_kind)
                                .get(&color_scheme_id)
                            {
                                match ron::ser::to_string_pretty(
                                    &color_scheme,
                                    ron::ser::PrettyConfig::new(),
                                ) {
                                    Ok(ron) => {
                                        if let Err(err) = fs::write(path, ron) {
                                            log::error!(
                                                "failed to export {:?} to {:?}: {}",
                                                color_scheme_id,
                                                path,
                                                err
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "failed to serialize color scheme {:?}: {}",
                                            color_scheme_id,
                                            err
                                        );
                                    }
                                }
                            } else {
                                log::error!("failed to find color scheme {:?}", color_scheme_id);
                            }
                        }
                        None => {
                            let name = format!("COSMIC {:?}", color_scheme_kind);
                            let color_scheme = match color_scheme_kind {
                                ColorSchemeKind::Dark => ColorScheme::from((
                                    name.as_str(),
                                    &terminal_theme::cosmic_dark(),
                                )),
                                ColorSchemeKind::Light => ColorScheme::from((
                                    name.as_str(),
                                    &terminal_theme::cosmic_light(),
                                )),
                            };
                            //TODO: do not duplicate code
                            match ron::ser::to_string_pretty(
                                &color_scheme,
                                ron::ser::PrettyConfig::new(),
                            ) {
                                Ok(ron) => {
                                    if let Err(err) = fs::write(path, ron) {
                                        log::error!(
                                            "failed to export {:?} to {:?}: {}",
                                            color_scheme.name,
                                            path,
                                            err
                                        );
                                    }
                                }
                                Err(err) => {
                                    log::error!(
                                        "failed to serialize color scheme {:?}: {}",
                                        color_scheme.name,
                                        err
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Message::ColorSchemeExpand(color_scheme_kind, color_scheme_id_opt) => {
                self.color_scheme_expanded = Some((color_scheme_kind, color_scheme_id_opt));
            }
            Message::ColorSchemeImport(color_scheme_kind) => {
                if self.dialog_opt.is_none() {
                    self.color_scheme_errors.clear();
                    let (dialog, command) = Dialog::new(
                        DialogKind::OpenMultipleFiles,
                        None,
                        Message::DialogMessage,
                        move |result| Message::ColorSchemeImportResult(color_scheme_kind, result),
                    );
                    self.dialog_opt = Some(dialog);
                    return command;
                }
            }
            Message::ColorSchemeImportResult(color_scheme_kind, result) => {
                self.dialog_opt = None;
                if let DialogResult::Open(paths) = result {
                    self.color_scheme_errors.clear();
                    for path in &paths {
                        let mut file = match fs::File::open(path) {
                            Ok(ok) => ok,
                            Err(err) => {
                                self.color_scheme_errors
                                    .push(format!("Failed to open {path:?}: {err}"));
                                continue;
                            }
                        };
                        match ron::de::from_reader::<_, ColorScheme>(&mut file) {
                            Ok(color_scheme) => {
                                // Get next color_scheme ID
                                let color_scheme_id = self
                                    .config
                                    .color_schemes(color_scheme_kind)
                                    .last_key_value()
                                    .map(|(id, _)| ColorSchemeId(id.0 + 1))
                                    .unwrap_or_default();
                                self.config
                                    .color_schemes_mut(color_scheme_kind)
                                    .insert(color_scheme_id, color_scheme);
                            }
                            Err(err) => {
                                self.color_scheme_errors
                                    .push(format!("Failed to parse {path:?}: {err}"));
                            }
                        }
                    }
                    return self.save_color_schemes(color_scheme_kind);
                }
            }
            Message::ColorSchemeRename(color_scheme_kind, color_scheme_id, color_scheme_name) => {
                self.color_scheme_expanded = None;
                let focus = self.color_scheme_renaming.is_none();
                self.color_scheme_renaming =
                    Some((color_scheme_kind, color_scheme_id, color_scheme_name));
                if focus {
                    return widget::text_input::focus(self.color_scheme_rename_id.clone());
                }
            }
            Message::ColorSchemeRenameSubmit => {
                if let Some((color_scheme_kind, color_scheme_id, color_scheme_name)) =
                    self.color_scheme_renaming.take()
                {
                    if let Some(color_scheme) = self
                        .config
                        .color_schemes_mut(color_scheme_kind)
                        .get_mut(&color_scheme_id)
                    {
                        color_scheme.name = color_scheme_name;
                        return self.save_color_schemes(color_scheme_kind);
                    }
                }
            }
            Message::ColorSchemeTabActivate(entity) => {
                if let Some(color_scheme_kind) =
                    self.color_scheme_tab_model.data::<ColorSchemeKind>(entity)
                {
                    let context_page = ContextPage::ColorSchemes(*color_scheme_kind);
                    if self.context_page != context_page {
                        return self.update(Message::ToggleContextPage(context_page));
                    }
                }
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
                            return Task::batch([clipboard::write(text), self.update_focus()]);
                        }
                    }
                } else {
                    log::warn!("Failed to get focused pane");
                }
                return self.update_focus();
            }
            Message::CopyOrSigint(entity_opt) => {
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        let term = terminal.term.lock();
                        if let Some(text) = term.selection_to_string() {
                            return Task::batch([clipboard::write(text), self.update_focus()]);
                        } else {
                            // Drop the lock for term so that input_scroll doesn't block forever
                            drop(term);
                            // 0x03 is ^C
                            terminal.input_scroll(b"\x03".as_slice());
                        }
                    }
                } else {
                    log::warn!("Failed to get focused pane");
                }
                return self.update_focus();
            }
            Message::CopyPrimary(entity_opt) => {
                if let Some(tab_model) = self.pane_model.active() {
                    let entity = entity_opt.unwrap_or_else(|| tab_model.active());
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let terminal = terminal.lock().unwrap();
                        let term = terminal.term.lock();
                        if let Some(text) = term.selection_to_string() {
                            return Task::batch([
                                clipboard::write_primary(text),
                                self.update_focus(),
                            ]);
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

                            config_set!(font_name, font_name.to_string());
                            self.set_curr_font_weights_and_stretches();

                            return self.update_config();
                        }
                    }
                    None => {
                        log::warn!("failed to find font with index {}", index);
                    }
                }
            }
            Message::DefaultFontSize(index) => match self.font_sizes.get(index) {
                Some(font_size) => {
                    config_set!(font_size, *font_size);
                    self.reset_terminal_panes_zoom(); // reset zoom
                    return self.update_config();
                }
                None => {
                    log::warn!("failed to find font with index {}", index);
                }
            },
            Message::DefaultFontStretch(index) => match self.curr_font_stretches.get(index) {
                Some(font_stretch) => {
                    config_set!(font_stretch, font_stretch.to_number());
                    self.set_curr_font_weights_and_stretches();
                    return self.update_config();
                }
                None => {
                    log::warn!("failed to find font weight with index {}", index);
                }
            },
            Message::DefaultFontWeight(index) => match self.curr_font_weights.get(index) {
                Some(font_weight) => {
                    config_set!(font_weight, *font_weight);
                    return self.update_config();
                }
                None => {
                    log::warn!("failed to find font weight with index {}", index);
                }
            },
            Message::DefaultDimFontWeight(index) => match self.curr_font_weights.get(index) {
                Some(font_weight) => {
                    config_set!(dim_font_weight, *font_weight);
                    return self.update_config();
                }
                None => {
                    log::warn!("failed to find dim font weight with index {}", index);
                }
            },
            Message::DefaultBoldFontWeight(index) => match self.curr_font_weights.get(index) {
                Some(font_weight) => {
                    config_set!(bold_font_weight, *font_weight);
                    return self.update_config();
                }
                None => {
                    log::warn!("failed to find bold font weight with index {}", index);
                }
            },
            Message::DefaultZoomStep(index) => match self.zoom_steps.get(index) {
                Some(zoom_step) => {
                    config_set!(font_size_zoom_step_mul_100, *zoom_step);
                    self.reset_terminal_panes_zoom(); // reset zoom
                    return self.update_config();
                }
                None => {
                    log::warn!("failed to find zoom step with index {}", index);
                }
            },
            Message::DialogMessage(dialog_message) => {
                if let Some(dialog) = &mut self.dialog_opt {
                    return dialog.update(dialog_message);
                }
            }
            Message::Drop(Some((pane, entity, data))) => {
                self.pane_model.focus = pane;
                if let Ok(value) = shlex::try_join(data.paths.iter().filter_map(|p| p.to_str())) {
                    return Task::batch([
                        self.update_focus(),
                        command::message::app(Message::PasteValue(Some(entity), value)),
                    ]);
                }
            }
            Message::Drop(None) => {}
            Message::Find(find) => {
                self.find = find;
                if find {
                    if let Some(tab_model) = self.pane_model.active() {
                        let entity = tab_model.active();
                        if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                            let terminal = terminal.lock().unwrap();
                            let term = terminal.term.lock();
                            if let Some(text) = term.selection_to_string() {
                                self.find_search_value = text;
                            }
                        }
                    } else {
                        log::warn!("Failed to get focused pane");
                    }
                } else {
                    self.find_search_value.clear();
                }

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
            Message::MiddleClick(pane, entity_opt) => {
                self.pane_model.focus = pane;
                return Task::batch([
                    self.update_focus(),
                    clipboard::read_primary().map(move |value_opt| match value_opt {
                        Some(value) => message::app(Message::PasteValue(entity_opt, value)),
                        None => message::none(),
                    }),
                ]);
            }
            Message::FocusFollowMouse(focus_follow_mouse) => {
                config_set!(focus_follow_mouse, focus_follow_mouse);
            }
            Message::Key(modifiers, key) => {
                for (key_bind, action) in &self.key_binds {
                    if key_bind.matches(modifiers, &key) {
                        return self.update(action.message(None));
                    }
                }
            }
            Message::LaunchUrl(url) => {
                if let Err(err) = open::that_detached(&url) {
                    log::warn!("failed to open {:?}: {}", url, err);
                }
            }
            Message::Modifiers(modifiers) => {
                self.modifiers = modifiers;
            }
            Message::MouseEnter(pane) => {
                self.pane_model.focus = pane;
                return self.update_focus();
            }
            Message::Opacity(opacity) => {
                config_set!(opacity, cmp::min(100, opacity));
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
                    let command =
                        self.create_and_focus_new_terminal(pane, self.get_default_profile());
                    self.pane_model.panes_created += 1;
                    return command;
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
                return clipboard::read().map(move |value_opt| match value_opt {
                    Some(value) => message::app(Message::PasteValue(entity_opt, value)),
                    None => message::none(),
                });
            }
            Message::PastePrimary(entity_opt) => {
                return clipboard::read_primary().map(move |value_opt| match value_opt {
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
                return self.update_focus();
            }
            Message::ProfileCollapse(_profile_id) => {
                self.profile_expanded = None;
            }
            Message::ProfileCommand(profile_id, text) => {
                if let Some(profile) = self.config.profiles.get_mut(&profile_id) {
                    profile.command = text;
                    return self.save_profiles();
                }
            }
            Message::ProfileDirectory(profile_id, text) => {
                if let Some(profile) = self.config.profiles.get_mut(&profile_id) {
                    profile.working_directory = text;
                    return self.save_profiles();
                }
            }
            Message::ProfileExpand(profile_id) => {
                self.profile_expanded = Some(profile_id);
            }
            Message::ProfileHold(profile_id, hold) => {
                if let Some(profile) = self.config.profiles.get_mut(&profile_id) {
                    profile.hold = hold;
                    return self.save_profiles();
                }
            }
            Message::ProfileName(profile_id, text) => {
                if let Some(profile) = self.config.profiles.get_mut(&profile_id) {
                    profile.name = text;
                    return self.save_profiles();
                }
            }
            Message::ProfileNew => {
                // Get next profile ID
                let profile_id = self
                    .config
                    .profiles
                    .last_key_value()
                    .map(|(id, _)| ProfileId(id.0 + 1))
                    .unwrap_or_default();
                self.config.profiles.insert(profile_id, Profile::default());
                self.profile_expanded = Some(profile_id);
                return self.save_profiles();
            }
            Message::ProfileOpen(profile_id) => {
                return self.create_and_focus_new_terminal(self.pane_model.focus, Some(profile_id));
            }
            Message::ProfileRemove(profile_id) => {
                // Reset matching terminals to default profile
                for (_pane, tab_model) in self.pane_model.panes.iter() {
                    for entity in tab_model.iter() {
                        if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                            let mut terminal = terminal.lock().unwrap();
                            if terminal.profile_id_opt == Some(profile_id) {
                                terminal.profile_id_opt = None;
                            }
                        }
                    }
                }
                if Some(profile_id) == self.get_default_profile() {
                    config_set!(default_profile, None);
                }
                self.config.profiles.remove(&profile_id);
                return self.save_profiles();
            }
            Message::ProfileSyntaxTheme(profile_id, color_scheme_kind, theme_i) => {
                match self
                    .theme_names(color_scheme_kind)
                    .get(theme_i)
                    .map(|x| x.to_string())
                {
                    Some(theme_name) => {
                        if let Some(profile) = self.config.profiles.get_mut(&profile_id) {
                            match color_scheme_kind {
                                ColorSchemeKind::Dark => {
                                    profile.syntax_theme_dark = theme_name;
                                }
                                ColorSchemeKind::Light => {
                                    profile.syntax_theme_light = theme_name;
                                }
                            }
                            return self.save_profiles();
                        }
                    }
                    None => {
                        log::warn!("failed to find syntax theme with index {}", theme_i);
                    }
                }
            }
            Message::ProfileTabTitle(profile_id, text) => {
                if let Some(profile) = self.config.profiles.get_mut(&profile_id) {
                    profile.tab_title = text;
                    return self.save_profiles();
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
                return self.update_focus();
            }
            Message::ShowHeaderBar(show_headerbar) => {
                if show_headerbar != self.config.show_headerbar {
                    config_set!(show_headerbar, show_headerbar);
                    return self.update_config();
                }
            }
            Message::UseBrightBold(use_bright_bold) => {
                if use_bright_bold != self.config.use_bright_bold {
                    config_set!(use_bright_bold, use_bright_bold);
                    return self.update_config();
                }
            }
            Message::ShowAdvancedFontSettings(show) => {
                self.show_advanced_font_settings = show;
            }
            Message::SystemThemeChange => {
                return self.update_config();
            }
            Message::SyntaxTheme(color_scheme_kind, index) => {
                match self.theme_names(color_scheme_kind).get(index) {
                    Some(theme_name) => {
                        match color_scheme_kind {
                            ColorSchemeKind::Dark => {
                                config_set!(syntax_theme_dark, theme_name.to_string());
                            }
                            ColorSchemeKind::Light => {
                                config_set!(syntax_theme_light, theme_name.to_string());
                            }
                        }
                        return self.update_config();
                    }
                    None => {
                        log::warn!("failed to find syntax theme with index {}", index);
                    }
                }
            }
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
                            if let Some(window_id) = self.core.main_window_id() {
                                return window::close(window_id);
                            }
                        }
                    }
                }

                return self.update_title(None);
            }
            Message::TabContextAction(entity, action) => {
                if let Some(tab_model) = self.pane_model.active() {
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        // Close context menu
                        {
                            let mut terminal = terminal.lock().unwrap();
                            terminal.context_menu = None;
                        }
                        // Run action's message
                        return self.update(action.message(Some(entity)));
                    }
                }
            }
            Message::TabContextMenu(pane, position_opt) => {
                // Close any existing context menues
                let panes: Vec<_> = self.pane_model.panes.iter().collect();
                for (_pane, tab_model) in panes {
                    let entity = tab_model.active();
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        let mut terminal = terminal.lock().unwrap();
                        terminal.context_menu = None;
                    }
                }

                // Show the context menu on the correct pane / terminal
                if let Some(tab_model) = self.pane_model.panes.get(pane) {
                    let entity = tab_model.active();
                    if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                        // Update context menu position
                        let mut terminal = terminal.lock().unwrap();
                        terminal.context_menu = position_opt;
                    }
                }

                // Shift focus to the pane / terminal
                // with the context menu
                self.pane_model.focus = pane;
                return self.update_title(Some(pane));
            }
            Message::TabNew => {
                return self.create_and_focus_new_terminal(
                    self.pane_model.focus,
                    self.get_default_profile(),
                )
            }
            Message::TabNewNoProfile => {
                return self.create_and_focus_new_terminal(self.pane_model.focus, None)
            }
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
                    TermEvent::ClipboardLoad(kind, callback) => {
                        match kind {
                            term::ClipboardType::Clipboard => {
                                log::info!("clipboard load");
                                return clipboard::read().map(move |data_opt| {
                                    //TODO: what to do when data_opt is None?
                                    callback(&data_opt.unwrap_or_default());
                                    // We don't need to do anything else
                                    message::none()
                                });
                            }
                            term::ClipboardType::Selection => {
                                log::info!("TODO: load selection");
                            }
                        }
                    }
                    TermEvent::ClipboardStore(kind, data) => match kind {
                        term::ClipboardType::Clipboard => {
                            log::info!("clipboard store");
                            return clipboard::write(data);
                        }
                        term::ClipboardType::Selection => {
                            log::info!("TODO: store selection");
                        }
                    },
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
                    TermEvent::CursorBlinkingChange => {
                        //TODO: should we blink the cursor?
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
                            let tab_title_override =
                                if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                                    let terminal = terminal.lock().unwrap();
                                    terminal.tab_title_override.clone()
                                } else {
                                    None
                                };
                            tab_model.text_set(
                                entity,
                                tab_title_override.unwrap_or_else(|| fl!("new-terminal")),
                            );
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
                            let has_override =
                                if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                                    let terminal = terminal.lock().unwrap();
                                    terminal.tab_title_override.is_some()
                                } else {
                                    false
                                };
                            if !has_override {
                                tab_model.text_set(entity, title);
                            }
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
                    TermEvent::ChildExit(_error_code) => {
                        //Ignore this for now
                    }
                }
            }
            Message::TermEventTx(term_event_tx) => {
                // Check if the terminal event channel was reset
                if self.term_event_tx_opt.is_some() {
                    // Close tabs using old terminal event channel
                    log::warn!("terminal event channel reset, closing tabs");

                    // First, close other panes
                    while let Some((_state, sibling)) =
                        self.pane_model.panes.close(self.pane_model.focus)
                    {
                        self.terminal_ids.remove(&self.pane_model.focus);
                        self.pane_model.focus = sibling;
                    }

                    // Next, close all tabs in the active pane
                    if let Some(tab_model) = self.pane_model.active_mut() {
                        let entities: Vec<_> = tab_model.iter().collect();
                        for entity in entities {
                            tab_model.remove(entity);
                        }
                    }
                }

                // Set new terminal event channel
                self.term_event_tx_opt = Some(term_event_tx);

                // Spawn first tab
                return self.update(Message::TabNew);
            }
            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }

                // Extra work to do to prepare context pages
                if let ContextPage::ColorSchemes(color_scheme_kind) = self.context_page {
                    self.color_scheme_errors.clear();
                    self.color_scheme_expanded = None;
                    self.color_scheme_renaming = None;
                    self.color_scheme_tab_model = widget::segmented_button::Model::default();
                    let dark_entity = self
                        .color_scheme_tab_model
                        .insert()
                        .text(fl!("dark"))
                        .data(ColorSchemeKind::Dark)
                        .id();
                    let light_entity = self
                        .color_scheme_tab_model
                        .insert()
                        .text(fl!("light"))
                        .data(ColorSchemeKind::Light)
                        .id();
                    self.color_scheme_tab_model
                        .activate(match color_scheme_kind {
                            ColorSchemeKind::Dark => dark_entity,
                            ColorSchemeKind::Light => light_entity,
                        });
                }

                self.set_context_title(context_page.title());
            }
            Message::UpdateDefaultProfile((default, profile_id)) => {
                config_set!(default_profile, default.then_some(profile_id));
            }
            Message::WindowClose => {
                if let Some(window_id) = self.core.main_window_id() {
                    return window::close(window_id);
                }
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
                return self.update_render_active_pane_zoom(message);
            }
            Message::ZoomOut => {
                return self.update_render_active_pane_zoom(message);
            }
            Message::ZoomReset => {
                self.reset_terminal_panes_zoom();
                return self.update_config();
            }
        }

        Task::none()
    }

    fn context_drawer(&self) -> Option<Element<Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => self.about(),
            ContextPage::ColorSchemes(color_scheme_kind) => self.color_schemes(color_scheme_kind),
            ContextPage::Profiles => self.profiles(),
            ContextPage::Settings => self.settings(),
        })
    }

    fn header_start(&self) -> Vec<Element<Self::Message>> {
        vec![menu_bar(&self.config, &self.key_binds)]
    }

    fn header_end(&self) -> Vec<Element<Self::Message>> {
        vec![
            widget::button::custom(icon_cache_get("list-add-symbolic", 16))
                .on_press(Message::TabNew)
                .padding(8)
                .class(style::Button::Icon)
                .into(),
        ]
    }

    fn view_window(&self, window_id: window::Id) -> Element<Message> {
        match &self.dialog_opt {
            Some(dialog) => dialog.view(window_id),
            None => widget::text("Unknown window ID").into(),
        }
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = self.core().system_theme().cosmic().spacing;

        let pane_grid = PaneGrid::new(&self.pane_model.panes, |pane, tab_model, _is_maximized| {
            let mut tab_column = widget::column::with_capacity(1);

            if tab_model.iter().count() > 1 {
                tab_column = tab_column.push(
                    widget::container(
                        widget::tab_bar::horizontal(tab_model)
                            .button_height(32)
                            .button_spacing(space_xxs)
                            .on_activate(Message::TabActivate)
                            .on_close(|entity| Message::TabClose(Some(entity))),
                    )
                    .class(style::Container::Background)
                    .width(Length::Fill),
                );
            }

            let entity = tab_model.active();
            let entity_middle_click = tab_model.active();
            let terminal_id = self
                .terminal_ids
                .get(&pane)
                .cloned()
                .unwrap_or_else(widget::Id::unique);
            if let Some(terminal) = tab_model.data::<Mutex<Terminal>>(entity) {
                let mut terminal_box = terminal_box(terminal)
                    .id(terminal_id)
                    .on_context_menu(move |position_opt| {
                        Message::TabContextMenu(pane, position_opt)
                    })
                    .on_middle_click(move || Message::MiddleClick(pane, Some(entity_middle_click)))
                    .opacity(self.config.opacity_ratio())
                    .padding(space_xxs);

                if self.config.focus_follow_mouse {
                    terminal_box = terminal_box.on_mouse_enter(move || Message::MouseEnter(pane));
                }

                let context_menu = {
                    let terminal = terminal.lock().unwrap();
                    terminal.context_menu
                };

                let tab_element: Element<'_, Message> = match context_menu {
                    Some(point) => widget::popover(terminal_box.context_menu(point))
                        .popup(menu::context_menu(&self.config, &self.key_binds, entity))
                        .position(widget::popover::Position::Point(point))
                        .into(),
                    None => terminal_box.into(),
                };
                tab_column = tab_column.push(tab_element);
            }

            //Only draw find in the currently focused pane
            if self.find && pane == self.pane_model.focus {
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
                    button::custom(icon_cache_get("edit-clear-symbolic", 16))
                        .on_press(Message::FindSearchValueChanged(String::new()))
                        .class(style::Button::Icon)
                        .into(),
                );
                let find_widget = widget::row::with_children(vec![
                    find_input.into(),
                    widget::tooltip(
                        button::custom(icon_cache_get("go-up-symbolic", 16))
                            .on_press(Message::FindPrevious)
                            .padding(space_xxs)
                            .class(style::Button::Icon),
                        widget::text::body(fl!("find-previous")),
                        widget::tooltip::Position::Top,
                    )
                    .into(),
                    widget::tooltip(
                        button::custom(icon_cache_get("go-down-symbolic", 16))
                            .on_press(Message::FindNext)
                            .padding(space_xxs)
                            .class(style::Button::Icon),
                        widget::text::body(fl!("find-next")),
                        widget::tooltip::Position::Top,
                    )
                    .into(),
                    widget::horizontal_space().into(),
                    button::custom(icon_cache_get("window-close-symbolic", 16))
                        .on_press(Message::Find(false))
                        .padding(space_xxs)
                        .class(style::Button::Icon)
                        .into(),
                ])
                .align_y(Alignment::Center)
                .padding(space_xxs)
                .spacing(space_xxs);

                tab_column = tab_column
                    .push(widget::layer_container(find_widget).layer(cosmic_theme::Layer::Primary));
            } else {
                // TODO
            }

            DndDestination::for_data::<DndDrop>(tab_column, move |data, action| {
                if let Some(data) = data {
                    if action == DndAction::Move {
                        Message::Drop(Some((pane, entity, data)))
                    } else {
                        log::warn!("unsuppported action: {:?}", action);
                        Message::Drop(None)
                    }
                } else {
                    Message::Drop(None)
                }
            })
            .apply(pane_grid::Content::new)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .on_click(Message::PaneClicked)
        .on_resize(space_xxs, Message::PaneResized)
        .on_drag(Message::PaneDragged);

        //TODO: apply window border radius xs at bottom of window
        pane_grid.into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct TerminalEventSubscription;
        struct ThemeSubscription;
        struct ThemeModeSubscription;

        Subscription::batch([
            event::listen_with(|event, _status, _window_id| match event {
                Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) => {
                    Some(Message::Key(modifiers, key))
                }
                Event::Keyboard(KeyEvent::ModifiersChanged(modifiers)) => {
                    Some(Message::Modifiers(modifiers))
                }
                Event::Mouse(MouseEvent::ButtonReleased(MouseButton::Left)) => {
                    Some(Message::CopyPrimary(None))
                }
                _ => None,
            }),
            Subscription::run_with_id(
                TypeId::of::<TerminalEventSubscription>(),
                stream::channel(100, |mut output| async move {
                    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
                    output.send(Message::TermEventTx(event_tx)).await.unwrap();

                    while let Some((pane, entity, event)) = event_rx.recv().await {
                        output
                            .send(Message::TermEvent(pane, entity, event))
                            .await
                            .unwrap();
                    }

                    panic!("terminal event channel closed");
                }),
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
            cosmic_config::config_subscription::<_, cosmic_theme::Theme>(
                TypeId::of::<ThemeSubscription>(),
                if self.core.system_theme_mode().is_dark {
                    cosmic_theme::DARK_THEME_ID
                } else {
                    cosmic_theme::LIGHT_THEME_ID
                }
                .into(),
                cosmic_theme::Theme::VERSION,
            )
            .map(|_update| Message::SystemThemeChange),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeModeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::VERSION,
            )
            .map(|_update| Message::SystemThemeChange),
            match &self.dialog_opt {
                Some(dialog) => dialog.subscription(),
                None => Subscription::none(),
            },
        ])
    }
}
