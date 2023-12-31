use alacritty_terminal::{
    event::{Event, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    grid::Dimensions,
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    sync::FairMutex,
    term::{
        cell::Flags,
        color::{self, Colors},
        Config,
        viewport_to_point, TermMode,
    },
    tty::{self, Options},
    Term,
    vte::ansi::{Color, NamedColor, Rgb},
};
use cosmic::{iced::advanced::graphics::text::font_system, widget::segmented_button};
use cosmic_text::{
    Attrs, AttrsList, Buffer, BufferLine, Family, Metrics, Shaping, Style, Weight, Wrap,
};
use std::{
    borrow::Cow,
    collections::HashMap,
    mem,
    sync::{Arc, Weak},
    time::Instant,
};
use tokio::sync::mpsc;

pub use alacritty_terminal::grid::Scroll as TerminalScroll;

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
    segmented_button::Entity,
    mpsc::Sender<(segmented_button::Entity, Event)>,
);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        //TODO: handle error
        let _ = self.1.blocking_send((self.0, event));
    }
}

fn convert_color(colors: &Colors, color: Color) -> cosmic_text::Color {
    let rgb = match color {
        Color::Named(named_color) => match colors[named_color] {
            Some(rgb) => rgb,
            None => {
                log::warn!("missing named color {:?}", named_color);
                Rgb::default()
            }
        },
        Color::Spec(rgb) => rgb,
        Color::Indexed(index) => match colors[index as usize] {
            Some(rgb) => rgb,
            None => {
                log::warn!("missing indexed color {}", index);
                Rgb::default()
            }
        },
    };
    cosmic_text::Color::rgb(rgb.r, rgb.g, rgb.b)
}

pub struct Terminal {
    default_attrs: Attrs<'static>,
    buffer: Arc<Buffer>,
    size: Size,
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    colors: Colors,
    notifier: Notifier,
    pub context_menu: Option<cosmic::iced::Point>,
}

impl Terminal {
    //TODO: error handling
    pub fn new(
        entity: segmented_button::Entity,
        event_tx: mpsc::Sender<(segmented_button::Entity, Event)>,
        config: Config,
        colors: Colors,
    ) -> Self {
        let metrics = Metrics::new(14.0, 20.0);
        //TODO: set color to default fg
        let default_attrs = Attrs::new()
            .family(Family::Monospace)
            .color(convert_color(&colors, Color::Named(NamedColor::Foreground)))
            .metadata(convert_color(&colors, Color::Named(NamedColor::Background)).0 as usize);
        let mut buffer = Buffer::new_empty(metrics);

        let (cell_width, cell_height) = {
            let mut font_system = font_system().write().unwrap();
            let mut font_system = font_system.raw();
            buffer.set_wrap(&mut font_system, Wrap::None);

            // Use size of space to determine cell size
            buffer.set_text(&mut font_system, " ", default_attrs, Shaping::Advanced);
            let layout = buffer.line_layout(&mut font_system, 0).unwrap();
            (layout[0].w, metrics.line_height)
        };

        let size = Size {
            width: (80.0 * cell_width).ceil() as u32,
            height: (24.0 * cell_height).ceil() as u32,
            cell_width,
            cell_height,
        };
        let event_proxy = EventProxy(entity, event_tx);
        let term = Arc::new(FairMutex::new(Term::new(
            config,
            &size,
            event_proxy.clone(),
        )));

        let window_id = 0;
        let options = Options::default();

        let pty = tty::new(&options, size.into(), window_id).unwrap();

        let pty_event_loop = EventLoop::new(
            term.clone(),
            event_proxy,
            pty,
            options.hold,
            false,
        );
        let notifier = Notifier(pty_event_loop.channel());
        let _pty_join_handle = pty_event_loop.spawn();

        Self {
            colors,
            default_attrs,
            buffer: Arc::new(buffer),
            size,
            term,
            notifier,
            context_menu: None,
        }
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
                buffer.set_size(font_system.raw(), width as f32, height as f32);
            });

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

    pub fn select_all(&mut self) {
        {
            let mut term = self.term.lock();
            let grid = term.grid();
            let start = Point::new(Line(-(grid.history_size() as i32)), Column(0));
            let end = Point::new(
                Line(grid.screen_lines() as i32 - 1),
                Column(grid.columns() - 1),
            );
            let mut selection = Selection::new(SelectionType::Lines, start, Side::Left);
            selection.update(end, Side::Right);
            term.selection = Some(selection);
        }
        self.update();
    }

    pub fn set_config(&mut self, config: &crate::Config, themes: &HashMap<String, Colors>) {
        let mut update_cell_size = false;
        let mut update = false;

        let metrics = config.metrics();
        if metrics != self.buffer.metrics() {
            {
                let mut font_system = font_system().write().unwrap();
                self.with_buffer_mut(|buffer| buffer.set_metrics(font_system.raw(), metrics));
            }
            update_cell_size = true;
        }

        if let Some(colors) = themes.get(config.syntax_theme()) {
            let mut changed = false;
            for i in 0..color::COUNT {
                if self.colors[i] != colors[i] {
                    self.colors[i] = colors[i];
                    changed = true;
                }
            }
            if changed {
                self.default_attrs = Attrs::new()
                    .family(Family::Monospace)
                    .color(convert_color(&colors, Color::Named(NamedColor::Foreground)))
                    .metadata(
                        convert_color(&colors, Color::Named(NamedColor::Background)).0 as usize,
                    );
                update = true;
            }
        }

        if update_cell_size {
            self.update_cell_size();
        } else if update {
            self.update();
        }
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
                (layout[0].w, buffer.metrics().line_height)
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
        let instant = Instant::now();

        //TODO: is redraw needed after all events?
        //TODO: use LineDamageBounds
        {
            let buffer = Arc::make_mut(&mut self.buffer);

            let mut line_i = 0;
            let mut last_point = None;
            let mut text = String::new();
            let mut attrs_list = AttrsList::new(self.default_attrs);
            {
                let term = self.term.lock();
                let grid = term.grid();
                for indexed in grid.display_iter() {
                    if indexed.point.line != last_point.unwrap_or(indexed.point).line {
                        while line_i >= buffer.lines.len() {
                            buffer.lines.push(BufferLine::new(
                                "",
                                AttrsList::new(self.default_attrs),
                                Shaping::Advanced,
                            ));
                            buffer.set_redraw(true);
                        }

                        if buffer.lines[line_i].set_text(text.clone(), attrs_list.clone()) {
                            buffer.set_redraw(true);
                        }
                        line_i += 1;

                        text.clear();
                        attrs_list.clear_spans();
                    }
                    //TODO: use indexed.point.column?

                    //TODO: skip leading spacer?
                    if indexed.cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                        // Skip wide spacers (cells after wide characters)
                        continue;
                    }

                    let start = text.len();
                    text.push(indexed.cell.c);
                    if let Some(zerowidth) = indexed.cell.zerowidth() {
                        for &c in zerowidth {
                            text.push(c);
                        }
                    }
                    let end = text.len();

                    let mut attrs = self.default_attrs;

                    let (mut fg, mut bg) = if indexed.cell.flags.contains(Flags::INVERSE) {
                        (
                            convert_color(&self.colors, indexed.cell.bg),
                            convert_color(&self.colors, indexed.cell.fg),
                        )
                    } else {
                        (
                            convert_color(&self.colors, indexed.cell.fg),
                            convert_color(&self.colors, indexed.cell.bg),
                        )
                    };

                    // Change color if cursor
                    if indexed.point == grid.cursor.point {
                        //TODO: better handling of cursor
                        mem::swap(&mut fg, &mut bg);
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

                    attrs = attrs.color(fg);
                    // Use metadata as background color
                    attrs = attrs.metadata(bg.0 as usize);
                    //TODO: more flags
                    if indexed.cell.flags.contains(Flags::BOLD) {
                        attrs = attrs.weight(Weight::BOLD);
                    }
                    if indexed.cell.flags.contains(Flags::ITALIC) {
                        attrs = attrs.style(Style::Italic);
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
                    AttrsList::new(self.default_attrs),
                    Shaping::Advanced,
                ));
                buffer.set_redraw(true);
            }

            if buffer.lines[line_i].set_text(text, attrs_list) {
                buffer.set_redraw(true);
            }
            line_i += 1;

            if buffer.lines.len() != line_i {
                buffer.lines.truncate(line_i);
                buffer.set_redraw(true);
            }

            {
                let mut font_system = font_system().write().unwrap();
                buffer.shape_until_scroll(font_system.raw(), true);
            }
        }

        log::debug!("buffer update {:?}", instant.elapsed());

        self.buffer.redraw()
    }

    pub fn viewport_to_point(&self, point: Point<usize>) -> Point {
        let term = self.term.lock();
        viewport_to_point(term.grid().display_offset(), point)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Ensure shutdown on terminal drop
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}
