use alacritty_terminal::{
    event::{Event, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    grid::Dimensions,
    index::{Boundary, Column, Direction, Line, Point, Side},
    selection::{Selection, SelectionType},
    sync::FairMutex,
    term::{
        cell::Flags,
        color::{self, Colors},
        search::RegexSearch,
        viewport_to_point, Config, TermDamage, TermMode,
    },
    tty::{self, Options},
    vte::ansi::{Color, NamedColor, Rgb},
    Term,
};
use cosmic::{
    iced::advanced::graphics::text::font_system,
    iced::mouse::ScrollDelta,
    widget::{pane_grid, segmented_button},
};
use cosmic_text::{
    Attrs, AttrsList, Buffer, BufferLine, CacheKeyFlags, Family, LineEnding, Metrics, Shaping,
    Weight, Wrap,
};
use indexmap::IndexSet;
use std::{
    borrow::Cow,
    collections::HashMap,
    io, mem,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Weak,
    },
    time::Instant,
};
use tokio::sync::mpsc;

pub use alacritty_terminal::grid::Scroll as TerminalScroll;

use crate::{
    config::{ColorSchemeKind, Config as AppConfig, ProfileId},
    mouse_reporter::MouseReporter,
};

/// Minimum contrast between a fixed cursor color and the cell's background.
/// Duplicated from alacritty
pub const MIN_CURSOR_CONTRAST: f64 = 1.5;

#[derive(Clone, Copy, Debug)]
pub struct Size {
    pub width: u32,
    pub height: u32,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        ((self.height as f32) / self.cell_height).floor() as usize
    }

    fn columns(&self) -> usize {
        ((self.width as f32) / self.cell_width).floor() as usize
    }
}

impl From<Size> for WindowSize {
    fn from(size: Size) -> Self {
        Self {
            num_lines: size.screen_lines() as u16,
            num_cols: size.columns() as u16,
            cell_width: size.cell_width as u16,
            cell_height: size.cell_height as u16,
        }
    }
}

#[derive(Clone)]
pub struct EventProxy(
    pane_grid::Pane,
    segmented_button::Entity,
    mpsc::Sender<(pane_grid::Pane, segmented_button::Entity, Event)>,
);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        //TODO: handle error
        let _ = self.2.blocking_send((self.0, self.1, event));
    }
}

fn as_bright(mut color: Color) -> Color {
    if let Color::Named(named) = color {
        color = Color::Named(named.to_bright());
    }
    color
}

fn as_dim(mut color: Color) -> Color {
    if let Color::Named(named) = color {
        color = Color::Named(named.to_dim());
    }
    color
}

pub static WINDOW_BG_COLOR: AtomicU32 = AtomicU32::new(0xFF000000);

fn convert_color(colors: &Colors, color: Color) -> cosmic_text::Color {
    let rgb = match color {
        Color::Named(named_color) => match colors[named_color] {
            Some(rgb) => rgb,
            None => {
                if named_color == NamedColor::Background {
                    // Allow using an unset background
                    return cosmic_text::Color(WINDOW_BG_COLOR.load(Ordering::SeqCst));
                } else {
                    log::warn!("missing named color {:?}", named_color);
                    Rgb::default()
                }
            }
        },
        Color::Spec(rgb) => rgb,
        Color::Indexed(index) => {
            if let Some(rgb) = colors[index as usize] {
                rgb
            } else {
                log::warn!("missing indexed color {}", index);
                Rgb::default()
            }
        }
    };
    cosmic_text::Color::rgb(rgb.r, rgb.g, rgb.b)
}

type TabModel = segmented_button::Model<segmented_button::SingleSelect>;
pub struct TerminalPaneGrid {
    pub panes: pane_grid::State<TabModel>,
    pub panes_created: usize,
    pub focus: pane_grid::Pane,
}

impl TerminalPaneGrid {
    pub fn new(model: TabModel) -> Self {
        let (panes, pane) = pane_grid::State::new(model);
        let mut terminal_ids = HashMap::new();
        terminal_ids.insert(pane, cosmic::widget::Id::unique());

        Self {
            panes,
            panes_created: 1,
            focus: pane,
        }
    }
    pub fn active(&self) -> Option<&TabModel> {
        self.panes.get(self.focus)
    }
    pub fn active_mut(&mut self) -> Option<&mut TabModel> {
        self.panes.get_mut(self.focus)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Metadata {
    pub bg: cosmic_text::Color,
    pub underline_color: cosmic_text::Color,
    pub flags: Flags,
}

impl Metadata {
    fn new(bg: cosmic_text::Color, underline_color: cosmic_text::Color) -> Self {
        let flags = Flags::empty();
        Self {
            bg,
            underline_color,
            flags,
        }
    }

    fn with_underline_color(self, underline_color: cosmic_text::Color) -> Self {
        Self {
            underline_color,
            ..self
        }
    }

    fn with_flags(self, flags: Flags) -> Self {
        Self { flags, ..self }
    }
}

pub struct Terminal {
    pub context_menu: Option<cosmic::iced::Point>,
    pub metadata_set: IndexSet<Metadata>,
    pub needs_update: bool,
    pub profile_id_opt: Option<ProfileId>,
    pub tab_title_override: Option<String>,
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    bold_font_weight: Weight,
    buffer: Arc<Buffer>,
    colors: Colors,
    default_attrs: Attrs<'static>,
    dim_font_weight: Weight,
    mouse_reporter: MouseReporter,
    notifier: Notifier,
    search_regex_opt: Option<RegexSearch>,
    search_value: String,
    size: Size,
    use_bright_bold: bool,
}

impl Terminal {
    //TODO: error handling
    pub fn new(
        pane: pane_grid::Pane,
        entity: segmented_button::Entity,
        event_tx: mpsc::Sender<(pane_grid::Pane, segmented_button::Entity, Event)>,
        config: Config,
        options: Options,
        app_config: &AppConfig,
        colors: Colors,
        profile_id_opt: Option<ProfileId>,
        tab_title_override: Option<String>,
    ) -> Result<Self, io::Error> {
        let font_stretch = app_config.typed_font_stretch();
        let font_weight = app_config.font_weight;
        let dim_font_weight = app_config.dim_font_weight;
        let bold_font_weight = app_config.bold_font_weight;
        let use_bright_bold = app_config.use_bright_bold;

        let metrics = Metrics::new(14.0, 20.0);

        let default_bg = convert_color(&colors, Color::Named(NamedColor::Background));
        let default_fg = convert_color(&colors, Color::Named(NamedColor::Foreground));

        let mut metadata_set = IndexSet::new();
        let default_metada = Metadata::new(default_bg, default_fg);
        let (default_metada_idx, _) = metadata_set.insert_full(default_metada);

        //TODO: set color to default fg
        let default_attrs = Attrs::new()
            .family(Family::Monospace)
            .weight(Weight(font_weight))
            .stretch(font_stretch)
            .color(default_fg)
            .metadata(default_metada_idx);

        let mut buffer = Buffer::new_empty(metrics);

        let (cell_width, cell_height) = {
            let mut font_system = font_system().write().unwrap();
            let font_system = font_system.raw();
            buffer.set_wrap(font_system, Wrap::None);

            // Use size of space to determine cell size
            buffer.set_text(font_system, " ", default_attrs, Shaping::Advanced);
            let layout = buffer.line_layout(font_system, 0).unwrap();
            let w = layout[0].w;
            buffer.set_monospace_width(font_system, Some(w));
            (w, metrics.line_height)
        };

        let size = Size {
            width: (80.0 * cell_width).ceil() as u32,
            height: (24.0 * cell_height).ceil() as u32,
            cell_width,
            cell_height,
        };
        let event_proxy = EventProxy(pane, entity, event_tx);
        let term = Arc::new(FairMutex::new(Term::new(
            config,
            &size,
            event_proxy.clone(),
        )));

        let window_id = 0;
        let pty = tty::new(&options, size.into(), window_id)?;

        let pty_event_loop = EventLoop::new(term.clone(), event_proxy, pty, options.hold, false)?;
        let notifier = Notifier(pty_event_loop.channel());
        let _pty_join_handle = pty_event_loop.spawn();

        Ok(Self {
            bold_font_weight: Weight(bold_font_weight),
            buffer: Arc::new(buffer),
            colors,
            context_menu: None,
            default_attrs,
            dim_font_weight: Weight(dim_font_weight),
            metadata_set,
            mouse_reporter: Default::default(),
            needs_update: true,
            notifier,
            profile_id_opt,
            search_regex_opt: None,
            search_value: String::new(),
            size,
            tab_title_override,
            term,
            use_bright_bold,
        })
    }

    pub fn buffer_weak(&self) -> Weak<Buffer> {
        Arc::downgrade(&self.buffer)
    }

    /// Get the internal [`Buffer`]
    pub fn with_buffer<F: FnOnce(&Buffer) -> T, T>(&self, f: F) -> T {
        f(&self.buffer)
    }

    /// Get the internal [`Buffer`], mutably
    pub fn with_buffer_mut<F: FnOnce(&mut Buffer) -> T, T>(&mut self, f: F) -> T {
        f(Arc::make_mut(&mut self.buffer))
    }

    pub fn colors(&self) -> &Colors {
        &self.colors
    }

    pub fn default_attrs(&self) -> &Attrs<'static> {
        &self.default_attrs
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn redraw(&self) -> bool {
        self.buffer.redraw()
    }

    pub fn set_redraw(&mut self, redraw: bool) {
        self.with_buffer_mut(|buffer| buffer.set_redraw(redraw));
    }

    pub fn input_no_scroll<I: Into<Cow<'static, [u8]>>>(&self, input: I) {
        self.notifier.notify(input);
    }

    pub fn input_scroll<I: Into<Cow<'static, [u8]>>>(&self, input: I) {
        self.input_no_scroll(input);
        self.scroll(TerminalScroll::Bottom);
    }

    pub fn paste(&self, value: String) {
        // This code is ported from alacritty
        let bracketed_paste = {
            let term = self.term.lock();
            term.mode().contains(TermMode::BRACKETED_PASTE)
        };
        if bracketed_paste {
            self.input_no_scroll(&b"\x1b[200~"[..]);
            self.input_no_scroll(value.replace('\x1b', "").into_bytes());
            self.input_scroll(&b"\x1b[201~"[..]);
        } else {
            // In non-bracketed (ie: normal) mode, terminal applications cannot distinguish
            // pasted data from keystrokes.
            // In theory, we should construct the keystrokes needed to produce the data we are
            // pasting... since that's neither practical nor sensible (and probably an impossible
            // task to solve in a general way), we'll just replace line breaks (windows and unix
            // style) with a single carriage return (\r, which is what the Enter key produces).
            self.input_scroll(value.replace("\r\n", "\r").replace('\n', "\r").into_bytes());
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width != self.size.width || height != self.size.height {
            let instant = Instant::now();

            self.size.width = width;
            self.size.height = height;

            self.notifier.on_resize(self.size.into());
            self.term.lock().resize(self.size);

            self.with_buffer_mut(|buffer| {
                let mut font_system = font_system().write().unwrap();
                buffer.set_size(font_system.raw(), Some(width as f32), Some(height as f32));
            });

            self.needs_update = true;

            log::debug!("resize {:?}", instant.elapsed());
        }
    }

    pub fn scroll(&self, scroll: TerminalScroll) {
        self.term.lock().scroll_display(scroll);
    }

    pub fn scroll_to(&self, ratio: f32) {
        let mut term = self.term.lock();
        let grid = term.grid();
        let total = grid.history_size() + grid.screen_lines();
        let old_display_offset = grid.display_offset() as i32;
        let new_display_offset =
            ((total as f32) * (1.0 - ratio)) as i32 - grid.screen_lines() as i32;
        term.scroll_display(TerminalScroll::Delta(
            new_display_offset - old_display_offset,
        ));
    }

    pub fn scrollbar(&self) -> Option<(f32, f32)> {
        let term = self.term.lock();
        let grid = term.grid();
        if grid.history_size() > 0 {
            let total = grid.history_size() + grid.screen_lines();
            let start = total - grid.display_offset() - grid.screen_lines();
            let end = total - grid.display_offset();
            Some((
                (start as f32) / (total as f32),
                (end as f32) / (total as f32),
            ))
        } else {
            None
        }
    }

    pub fn search(&mut self, value: &str, forwards: bool) {
        //TODO: set max lines, run in thread?
        {
            let mut term = self.term.lock();

            if self.search_value != value {
                match RegexSearch::new(value) {
                    Ok(search_regex) => {
                        self.search_regex_opt = Some(search_regex);
                        self.search_value = value.to_string();
                        term.selection = None;
                    }
                    Err(err) => {
                        log::warn!("failed to parse regex {:?}: {}", value, err);
                        return;
                    }
                }
            }

            let Some(search_regex) = &mut self.search_regex_opt else {
                return;
            };

            // Determine search origin
            let grid = term.grid();
            let search_origin = match term
                .selection
                .as_ref()
                .and_then(|selection| selection.to_range(&term))
            {
                Some(range) => {
                    //TODO: determine correct search_origin, along with side below
                    if forwards {
                        range.end.add(grid, Boundary::Grid, 1)
                    } else {
                        range.start.sub(grid, Boundary::Grid, 1)
                    }
                }
                None => {
                    if forwards {
                        Point::new(Line(-(grid.history_size() as i32)), Column(0))
                    } else {
                        Point::new(
                            Line(grid.screen_lines() as i32 - 1),
                            Column(grid.columns() - 1),
                        )
                    }
                }
            };

            // Find next search match
            if let Some(search_match) = term.search_next(
                search_regex,
                search_origin,
                if forwards {
                    Direction::Right
                } else {
                    Direction::Left
                },
                //TODO: determine correct side, along with search_origin above
                if forwards { Side::Left } else { Side::Right },
                None,
            ) {
                // Scroll to match
                if forwards {
                    term.scroll_to_point(*search_match.end());
                } else {
                    term.scroll_to_point(*search_match.start());
                }

                // Set selection to match
                let mut selection =
                    Selection::new(SelectionType::Simple, *search_match.start(), Side::Left);
                selection.update(*search_match.end(), Side::Right);
                term.selection = Some(selection);
            }
        }

        self.update();
    }

    pub fn select_all(&mut self) {
        {
            let mut term = self.term.lock();
            let grid = term.grid();
            let start = Point::new(Line(-(grid.history_size() as i32)), Column(0));
            let mut end_line = grid.bottommost_line();
            while end_line.0 > 0 {
                if !grid[end_line].is_clear() {
                    break;
                }
                end_line.0 -= 1;
            }
            let end = Point::new(end_line, Column(grid.columns() - 1));
            let mut selection = Selection::new(SelectionType::Lines, start, Side::Left);
            selection.update(end, Side::Right);
            term.selection = Some(selection);
        }
        self.update();
    }

    pub fn set_config(
        &mut self,
        config: &AppConfig,
        themes: &HashMap<(String, ColorSchemeKind), Colors>,
        zoom_adj: i8,
    ) {
        let mut update_cell_size = false;
        let mut update = false;

        if self.default_attrs.stretch != config.typed_font_stretch() {
            self.default_attrs = self.default_attrs.stretch(config.typed_font_stretch());
            update_cell_size = true;
        }

        if self.default_attrs.weight.0 != config.font_weight {
            self.default_attrs = self.default_attrs.weight(Weight(config.font_weight));
            update_cell_size = true;
        }

        if self.dim_font_weight.0 != config.dim_font_weight {
            self.dim_font_weight = Weight(config.dim_font_weight);
            update_cell_size = true;
        }

        if self.bold_font_weight.0 != config.font_weight {
            self.bold_font_weight = Weight(config.bold_font_weight);
            update_cell_size = true;
        }

        if self.use_bright_bold != config.use_bright_bold {
            self.use_bright_bold = config.use_bright_bold;
            update_cell_size = true;
        }

        let metrics = config.metrics(zoom_adj);
        if metrics != self.buffer.metrics() {
            {
                let mut font_system = font_system().write().unwrap();
                self.with_buffer_mut(|buffer| buffer.set_metrics(font_system.raw(), metrics));
            }
            update_cell_size = true;
        }

        if let Some(colors) = themes.get(&config.syntax_theme(self.profile_id_opt)) {
            let mut changed = false;
            for i in 0..color::COUNT {
                if self.colors[i] != colors[i] {
                    self.colors[i] = colors[i];
                    changed = true;
                }
            }
            if changed {
                update = true;
            }
        }

        // NOTE: this is done on every set_config because the changed boolean above does not capture
        // WINDOW_BG changes
        let default_colors_updated = self.update_default_colors(config);

        if update_cell_size {
            self.update_cell_size();
        } else if update || default_colors_updated {
            self.update();
        }
    }

    pub fn update_default_colors(&mut self, config: &AppConfig) -> bool {
        let default_bg = convert_color(&self.colors, Color::Named(NamedColor::Background));
        let default_fg = convert_color(&self.colors, Color::Named(NamedColor::Foreground));

        let new_default_metadata = Metadata::new(default_bg, default_fg);
        let curr_metada_idx = self.default_attrs().metadata;

        let updated = new_default_metadata != self.metadata_set[curr_metada_idx];

        if updated {
            self.metadata_set.clear();
            let (default_metadata_idx, _) = self.metadata_set.insert_full(new_default_metadata);

            self.default_attrs = Attrs::new()
                .family(Family::Monospace)
                .weight(Weight(config.font_weight))
                .stretch(config.typed_font_stretch())
                .color(default_fg)
                .metadata(default_metadata_idx);
        }

        updated
    }

    pub fn update_cell_size(&mut self) {
        let default_attrs = self.default_attrs;
        let (cell_width, cell_height) = {
            let mut font_system = font_system().write().unwrap();
            self.with_buffer_mut(|buffer| {
                buffer.set_wrap(font_system.raw(), Wrap::None);

                // Use size of space to determine cell size
                buffer.set_text(font_system.raw(), " ", default_attrs, Shaping::Advanced);
                let layout = buffer.line_layout(font_system.raw(), 0).unwrap();
                let w = layout[0].w;
                buffer.set_monospace_width(font_system.raw(), Some(w));
                (w, buffer.metrics().line_height)
            })
        };

        let old_size = self.size;
        self.size = Size {
            width: 0,
            height: 0,
            cell_width,
            cell_height,
        };
        self.resize(old_size.width, old_size.height);

        self.update();
    }

    pub fn update(&mut self) -> bool {
        // LEFT‑TO‑RIGHT ISOLATE character.
        // This will be added to the beginning of lines to force the shaper to treat detected RTL
        // lines as LTR. RTL text would still be rendered correctly. But this fixes the wrong
        // behavior of it being aligned to the right.
        const LRI: char = '\u{2066}';

        let instant = Instant::now();

        // Only keep default
        self.metadata_set.truncate(1);

        //TODO: is redraw needed after all events?
        //TODO: use LineDamageBounds
        {
            let buffer = Arc::make_mut(&mut self.buffer);

            let mut line_i = 0;
            let mut last_point = None;
            let mut text = String::from(LRI);
            let mut attrs_list = AttrsList::new(self.default_attrs);
            {
                let mut term = self.term.lock();
                //TODO: use damage?
                match term.damage() {
                    TermDamage::Full => {}
                    TermDamage::Partial(_damage_lines) => {}
                }
                term.reset_damage();

                let grid = term.grid();
                for indexed in grid.display_iter() {
                    if indexed.point.line != last_point.unwrap_or(indexed.point).line {
                        while line_i >= buffer.lines.len() {
                            buffer.lines.push(BufferLine::new(
                                "",
                                LineEnding::default(),
                                AttrsList::new(self.default_attrs),
                                Shaping::Advanced,
                            ));
                            buffer.set_redraw(true);
                        }

                        if buffer.lines[line_i].set_text(
                            text.clone(),
                            LineEnding::default(),
                            attrs_list.clone(),
                        ) {
                            buffer.set_redraw(true);
                        }
                        line_i += 1;

                        text.clear();
                        text.push(LRI);
                        attrs_list.clear_spans();
                    }
                    //TODO: use indexed.point.column?

                    //TODO: skip leading spacer?
                    if indexed.cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                        // Skip wide spacers (cells after wide characters)
                        continue;
                    }

                    let start = text.len();
                    // Tab skip/stop is handled by alacritty_terminal
                    text.push(match indexed.cell.c {
                        '\t' => ' ',
                        c => c,
                    });
                    if let Some(zerowidth) = indexed.cell.zerowidth() {
                        for &c in zerowidth {
                            text.push(c);
                        }
                    }
                    let end = text.len();

                    let mut attrs = self.default_attrs;

                    let cell_fg = if indexed.cell.flags.contains(Flags::DIM) {
                        as_dim(indexed.cell.fg)
                    } else if self.use_bright_bold && indexed.cell.flags.contains(Flags::BOLD) {
                        as_bright(indexed.cell.fg)
                    } else {
                        indexed.cell.fg
                    };

                    let (mut fg, mut bg) = if indexed.cell.flags.contains(Flags::INVERSE) {
                        (
                            convert_color(&self.colors, indexed.cell.bg),
                            convert_color(&self.colors, cell_fg),
                        )
                    } else {
                        (
                            convert_color(&self.colors, cell_fg),
                            convert_color(&self.colors, indexed.cell.bg),
                        )
                    };

                    if indexed.cell.flags.contains(Flags::HIDDEN) {
                        fg = bg;
                    }

                    // Change color if cursor
                    if indexed.point == grid.cursor.point {
                        //TODO: better handling of cursor
                        if term.mode().contains(TermMode::SHOW_CURSOR) {
                            //Use specific cursor color if requested
                            if term.colors()[NamedColor::Cursor].is_some() {
                                fg = bg;
                                bg = convert_color(term.colors(), Color::Named(NamedColor::Cursor));
                            } else if self.colors[NamedColor::Cursor].is_some() {
                                //Use specific theme cursor color if exists
                                fg = bg;
                                bg = convert_color(&self.colors, Color::Named(NamedColor::Cursor));
                            } else {
                                mem::swap(&mut fg, &mut bg);
                            }
                            let fg_rgb = Rgb {
                                r: fg.r(),
                                g: fg.g(),
                                b: fg.b(),
                            };
                            let bg_rgb = Rgb {
                                r: bg.r(),
                                g: bg.g(),
                                b: bg.b(),
                            };
                            let contrast = fg_rgb.contrast(bg_rgb);
                            if contrast < MIN_CURSOR_CONTRAST {
                                fg = convert_color(
                                    &self.colors,
                                    Color::Named(NamedColor::Background),
                                );
                                bg = convert_color(
                                    &self.colors,
                                    Color::Named(NamedColor::Foreground),
                                );
                            }
                        } else {
                            fg = bg;
                        }
                    }

                    // Change color if selected
                    if let Some(selection) = &term.selection {
                        if let Some(range) = selection.to_range(&term) {
                            if range.contains(indexed.point) {
                                //TODO: better handling of selection
                                mem::swap(&mut fg, &mut bg);
                            }
                        }
                    }

                    // Convert foreground to linear
                    attrs = attrs.color(fg);

                    let underline_color = indexed
                        .cell
                        .underline_color()
                        .map(|c| convert_color(&self.colors, c))
                        .unwrap_or(fg);
                    let metadata = Metadata::new(bg, fg)
                        .with_flags(indexed.cell.flags)
                        .with_underline_color(underline_color);
                    let (meta_idx, _) = self.metadata_set.insert_full(metadata);
                    attrs = attrs.metadata(meta_idx);

                    //TODO: more flags
                    if indexed.cell.flags.contains(Flags::BOLD) {
                        attrs = attrs.weight(self.bold_font_weight);
                    } else if indexed.cell.flags.contains(Flags::DIM) {
                        // if DIM and !BOLD
                        attrs = attrs.weight(self.dim_font_weight);
                    }
                    if indexed.cell.flags.contains(Flags::ITALIC) {
                        //TODO: automatically use fake italic
                        attrs = attrs.cache_key_flags(CacheKeyFlags::FAKE_ITALIC);
                    }
                    if attrs != attrs_list.defaults() {
                        attrs_list.add_span(start..end, attrs);
                    }

                    last_point = Some(indexed.point);
                }
            }

            //TODO: do not repeat!
            while line_i >= buffer.lines.len() {
                buffer.lines.push(BufferLine::new(
                    "",
                    LineEnding::default(),
                    AttrsList::new(self.default_attrs),
                    Shaping::Advanced,
                ));
                buffer.set_redraw(true);
            }

            if buffer.lines[line_i].set_text(text, LineEnding::default(), attrs_list) {
                buffer.set_redraw(true);
            }
            line_i += 1;

            if buffer.lines.len() != line_i {
                buffer.lines.truncate(line_i);
                buffer.set_redraw(true);
            }

            // Shape and trim shape run cache
            {
                let mut font_system = font_system().write().unwrap();
                buffer.shape_until_scroll(font_system.raw(), true);
                font_system.raw().shape_run_cache.trim(1024);
            }
        }

        log::debug!("buffer update {:?}", instant.elapsed());

        self.buffer.redraw()
    }

    pub fn viewport_to_point(&self, point: Point<usize>) -> Point {
        let term = self.term.lock();
        viewport_to_point(term.grid().display_offset(), point)
    }

    pub fn report_mouse(
        &mut self,
        event: cosmic::iced::Event,
        modifiers: &cosmic::iced::keyboard::Modifiers,
        x: u32,
        y: u32,
    ) {
        let term_lock = self.term.lock();
        let mode = term_lock.mode();
        #[allow(clippy::collapsible_else_if)]
        if mode.contains(TermMode::SGR_MOUSE) {
            if let Some(code) = self.mouse_reporter.sgr_mouse_code(event, modifiers, x, y) {
                self.input_no_scroll(code)
            }
        } else {
            if let Some(code) = self.mouse_reporter.normal_mouse_code(
                event,
                modifiers,
                mode.contains(TermMode::UTF8_MOUSE),
                x,
                y,
            ) {
                self.input_no_scroll(code)
            }
        }
    }
    pub fn scroll_mouse(
        &mut self,
        delta: ScrollDelta,
        modifiers: &cosmic::iced::keyboard::Modifiers,
        x: u32,
        y: u32,
    ) {
        let term_lock = self.term.lock();
        let mode = term_lock.mode();
        if mode.contains(TermMode::SGR_MOUSE) {
            MouseReporter::report_sgr_mouse_wheel_scroll(
                self,
                self.size().cell_width,
                self.size().cell_height,
                delta,
                modifiers,
                x,
                y,
            );
        } else {
            MouseReporter::report_mouse_wheel_as_arrows(
                self,
                self.size().cell_width,
                self.size().cell_height,
                delta,
            );
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Ensure shutdown on terminal drop
        if let Err(err) = self.notifier.0.send(Msg::Shutdown) {
            log::warn!("Failed to send shutdown message on dropped terminal: {err}");
        }
    }
}
