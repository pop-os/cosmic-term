use alacritty_terminal::{
    ansi::{Color, Handler, NamedColor},
    config::{Config, PtyConfig},
    event::{Event, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg, Notifier, State},
    grid::Dimensions,
    index::{Column, Line, Point},
    sync::FairMutex,
    term::{
        cell::Flags,
        color::{Colors, Rgb},
    },
    tty, Term,
};
use cosmic::{iced::advanced::graphics::text::font_system, widget::segmented_button};
use cosmic_text::{
    Attrs, AttrsList, Buffer, BufferLine, Family, FontSystem, Metrics, Shaping, Style, Weight, Wrap,
};
use std::{
    borrow::Cow,
    mem,
    sync::{Arc, Weak},
    thread::JoinHandle,
    time::Instant,
};
use tokio::sync::mpsc;

pub use alacritty_terminal::grid::Scroll as TerminalScroll;

#[derive(Clone, Copy, Debug)]
pub struct Size {
    width: u32,
    height: u32,
    cell_width: f32,
    cell_height: f32,
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
struct EventProxy(
    segmented_button::Entity,
    mpsc::Sender<(segmented_button::Entity, Event)>,
);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        //TODO: handle error
        let _ = self.1.blocking_send((self.0, event));
    }
}

fn colors() -> Colors {
    let mut colors = Colors::default();

    // These colors come from `ransid`: https://gitlab.redox-os.org/redox-os/ransid/-/blob/master/src/color.rs
    let encode_rgb = |r: u8, g: u8, b: u8| -> Rgb { Rgb { r, g, b } };
    for value in 0..=255 {
        let color = match value {
            /* Naive colors
            0 => encode_rgb(0x00, 0x00, 0x00),
            1 => encode_rgb(0x80, 0x00, 0x00),
            2 => encode_rgb(0x00, 0x80, 0x00),
            3 => encode_rgb(0x80, 0x80, 0x00),
            4 => encode_rgb(0x00, 0x00, 0x80),
            5 => encode_rgb(0x80, 0x00, 0x80),
            6 => encode_rgb(0x00, 0x80, 0x80),
            7 => encode_rgb(0xc0, 0xc0, 0xc0),
            8 => encode_rgb(0x80, 0x80, 0x80),
            9 => encode_rgb(0xff, 0x00, 0x00),
            10 => encode_rgb(0x00, 0xff, 0x00),
            11 => encode_rgb(0xff, 0xff, 0x00),
            12 => encode_rgb(0x00, 0x00, 0xff),
            13 => encode_rgb(0xff, 0x00, 0xff),
            14 => encode_rgb(0x00, 0xff, 0xff),
            15 => encode_rgb(0xff, 0xff, 0xff),
            */
            // Pop colors (from pop-desktop gsettings)
            0 => encode_rgb(51, 51, 51),
            1 => encode_rgb(204, 0, 0),
            2 => encode_rgb(78, 154, 6),
            3 => encode_rgb(196, 160, 0),
            4 => encode_rgb(52, 101, 164),
            5 => encode_rgb(117, 80, 123),
            6 => encode_rgb(6, 152, 154),
            7 => encode_rgb(211, 215, 207),
            8 => encode_rgb(136, 128, 124),
            9 => encode_rgb(241, 93, 34),
            10 => encode_rgb(115, 196, 143),
            11 => encode_rgb(255, 206, 81),
            12 => encode_rgb(72, 185, 199),
            13 => encode_rgb(173, 127, 168),
            14 => encode_rgb(52, 226, 226),
            15 => encode_rgb(238, 238, 236),
            /* Indexed colors */
            16..=231 => {
                let convert = |value: u8| -> u8 {
                    match value {
                        0 => 0,
                        _ => value * 0x28 + 0x28,
                    }
                };

                let r = convert((value - 16) / 36 % 6);
                let g = convert((value - 16) / 6 % 6);
                let b = convert((value - 16) % 6);
                encode_rgb(r, g, b)
            }
            232..=255 => {
                let gray = (value - 232) * 10 + 8;
                encode_rgb(gray, gray, gray)
            }
        };
        colors[value as usize] = Some(color);
    }

    // Set special colors
    // Pop colors (from pop-desktop gsettings)
    colors[NamedColor::Foreground] = Some(encode_rgb(242, 242, 242));
    colors[NamedColor::Background] = Some(encode_rgb(51, 51, 51));
    /*TODO
    colors[NamedColor::Cursor] = colors[NamedColor::];
    colors[NamedColor::DimBlack] = colors[NamedColor::];
    colors[NamedColor::DimRed] = colors[NamedColor::];
    colors[NamedColor::DimGreen] = colors[NamedColor::];
    colors[NamedColor::DimYellow] = colors[NamedColor::];
    colors[NamedColor::DimBlue] = colors[NamedColor::];
    colors[NamedColor::DimMagenta] = colors[NamedColor::];
    colors[NamedColor::DimCyan] = colors[NamedColor::];
    colors[NamedColor::DimWhite] = colors[NamedColor::];
    */
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    //TODO colors[NamedColor::DimForeground] = colors[NamedColor::];

    colors
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
    metrics: Metrics,
    default_attrs: Attrs<'static>,
    buffer: Arc<Buffer>,
    size: Size,
    term: Arc<FairMutex<Term<EventProxy>>>,
    colors: Colors,
    notifier: Notifier,
    pty_join_handle: JoinHandle<(EventLoop<tty::Pty, EventProxy>, State)>,
}

impl Terminal {
    //TODO: error handling
    pub fn new(
        entity: segmented_button::Entity,
        event_tx: mpsc::Sender<(segmented_button::Entity, Event)>,
    ) -> Self {
        let colors = colors();

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

        let config = Config::default();
        let mut size = Size {
            width: (80.0 * cell_width).ceil() as u32,
            height: (24.0 * cell_height).ceil() as u32,
            cell_width,
            cell_height,
        };
        let event_proxy = EventProxy(entity, event_tx);
        let term = Arc::new(FairMutex::new(Term::new(
            &config,
            &size,
            event_proxy.clone(),
        )));

        let window_id = 0;
        let pty = tty::new(&config.pty_config, size.into(), window_id).unwrap();

        let pty_event_loop = EventLoop::new(
            term.clone(),
            event_proxy,
            pty,
            config.pty_config.hold,
            false,
        );
        let notifier = Notifier(pty_event_loop.channel());
        let pty_join_handle = pty_event_loop.spawn();

        Self {
            colors,
            metrics,
            default_attrs,
            buffer: Arc::new(buffer),
            size,
            term,
            notifier,
            pty_join_handle,
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

    pub fn resize(&mut self, width: u32, height: u32) {
        if width != self.size.width || height != self.size.height {
            let instant = Instant::now();

            self.size.width = width;
            self.size.height = height;

            self.notifier.on_resize(self.size.into());
            self.term.lock().resize(self.size);

            self.with_buffer_mut(|buffer| {
                let mut font_system = font_system().write().unwrap();
                buffer.set_size(
                    font_system.raw(),
                    width as f32,
                    height as f32,
                );
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

    pub fn scrollbar(&self) -> (f32, f32) {
        let term = self.term.lock();
        let grid = term.grid();
        let total = grid.history_size() + grid.screen_lines();
        let start = total - grid.display_offset() - grid.screen_lines();
        let end = total - grid.display_offset();
        (
            (start as f32) / (total as f32),
            (end as f32) / (total as f32),
        )
    }

    pub fn update(&mut self) -> bool {
        let instant = Instant::now();

        //TODO: is redraw needed after all events?
        //TODO: use LineDamageBounds
        {
            let mut buffer = Arc::make_mut(&mut self.buffer);

            let mut line_i = 0;
            let mut last_point = None;
            let mut text = String::new();
            let mut attrs_list = AttrsList::new(self.default_attrs);
            {
                let term_guard = self.term.lock();
                let grid = term_guard.grid();
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
                    let mut fg = convert_color(&self.colors, indexed.cell.fg);
                    let mut bg = convert_color(&self.colors, indexed.cell.bg);
                    //TODO: better handling of cursor
                    if indexed.point == grid.cursor.point {
                        mem::swap(&mut fg, &mut bg);
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
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Ensure shutdown on terminal drop
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}
