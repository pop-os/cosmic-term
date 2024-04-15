// SPDX-License-Identifier: GPL-3.0-only

use alacritty_terminal::{
    index::{Column as TermColumn, Point as TermPoint, Side as TermSide},
    selection::{Selection, SelectionType},
    term::{cell::Flags, TermMode},
};
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::{
    cosmic_theme::palette::{blend::Compose, WithAlpha},
    iced::{
        advanced::graphics::text::Raw,
        event::{Event, Status},
        keyboard::{Event as KeyEvent, Key, Modifiers},
        mouse::{self, Button, Event as MouseEvent, ScrollDelta},
        Color, Element, Length, Padding, Point, Rectangle, Size, Vector,
    },
    iced_core::{
        clipboard::Clipboard,
        keyboard::key::Named,
        layout::{self, Layout},
        renderer::{self, Quad, Renderer as _},
        text::Renderer as _,
        widget::{
            self,
            operation::{self, Operation, OperationOutputWrapper},
            tree, Id, Widget,
        },
        Border, Shell,
    },
    theme::Theme,
    Renderer,
};
use cosmic_text::LayoutGlyph;
use indexmap::IndexSet;
use std::{
    cell::Cell,
    cmp,
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::{key_bind::key_binds, terminal::Metadata, Action, Terminal, TerminalScroll};

pub struct TerminalBox<'a, Message> {
    terminal: &'a Mutex<Terminal>,
    id: Option<Id>,
    border: Border,
    padding: Padding,
    click_timing: Duration,
    context_menu: Option<Point>,
    on_context_menu: Option<Box<dyn Fn(Option<Point>) -> Message + 'a>>,
    on_mouse_enter: Option<Box<dyn Fn() -> Message + 'a>>,
    opacity: Option<f32>,
    mouse_inside_boundary: Option<bool>,
    key_binds: HashMap<KeyBind, Action>,
}

impl<'a, Message> TerminalBox<'a, Message>
where
    Message: Clone,
{
    pub fn new(terminal: &'a Mutex<Terminal>) -> Self {
        Self {
            terminal,
            id: None,
            border: Border::default(),
            padding: Padding::new(0.0),
            click_timing: Duration::from_millis(500),
            context_menu: None,
            on_context_menu: None,
            on_mouse_enter: None,
            opacity: None,
            mouse_inside_boundary: None,
            key_binds: key_binds(),
        }
    }

    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn border<B: Into<Border>>(mut self, border: B) -> Self {
        self.border = border.into();
        self
    }

    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn click_timing(mut self, click_timing: Duration) -> Self {
        self.click_timing = click_timing;
        self
    }

    pub fn context_menu(mut self, position: Point) -> Self {
        self.context_menu = Some(position);
        self
    }

    pub fn on_context_menu(
        mut self,
        on_context_menu: impl Fn(Option<Point>) -> Message + 'a,
    ) -> Self {
        self.on_context_menu = Some(Box::new(on_context_menu));
        self
    }

    pub fn on_mouse_enter(mut self, on_mouse_enter: impl Fn() -> Message + 'a) -> Self {
        self.on_mouse_enter = Some(Box::new(on_mouse_enter));
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity);
        self
    }
}

pub fn terminal_box<Message>(terminal: &Mutex<Terminal>) -> TerminalBox<'_, Message>
where
    Message: Clone,
{
    TerminalBox::new(terminal)
}

impl<'a, Message> Widget<Message, cosmic::Theme, Renderer> for TerminalBox<'a, Message>
where
    Message: Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(Length::Fill).height(Length::Fill);

        let mut terminal = self.terminal.lock().unwrap();

        //TODO: set size?

        // Update if needed
        if terminal.needs_update {
            terminal.update();
            terminal.needs_update = false;
        }

        // Calculate layout lines
        terminal.with_buffer(|buffer| {
            let mut layout_lines = 0;
            for line in &buffer.lines {
                match line.layout_opt() {
                    Some(layout) => layout_lines += layout.len(),
                    None => (),
                }
            }

            let height = layout_lines as f32 * buffer.metrics().line_height;
            let size = Size::new(limits.max().width, height);

            layout::Node::new(limits.resolve(Length::Fill, Length::Fill, size))
        })
    }

    fn operate(
        &self,
        tree: &mut widget::Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation<OperationOutputWrapper<Message>>,
    ) {
        let state = tree.state.downcast_mut::<State>();

        operation.focusable(state, self.id.as_ref());
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor_position: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();

        if let Some(Dragging::Scrollbar { .. }) = &state.dragging {
            return mouse::Interaction::Idle;
        }

        if let Some(p) = cursor_position.position_in(layout.bounds()) {
            let terminal = self.terminal.lock().unwrap();
            let buffer_size = terminal.with_buffer(|buffer| buffer.size());

            let x = p.x - self.padding.left;
            let y = p.y - self.padding.top;
            if x >= 0.0 && x < buffer_size.0 && y >= 0.0 && y < buffer_size.1 {
                return mouse::Interaction::Text;
            }
        }

        mouse::Interaction::Idle
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let instant = Instant::now();

        let state = tree.state.downcast_ref::<State>();

        let cosmic_theme = theme.cosmic();
        let scrollbar_w = cosmic_theme.spacing.space_xxs as f32;

        let view_position = layout.position() + [self.padding.left, self.padding.top].into();
        let view_w = cmp::min(viewport.width as i32, layout.bounds().width as i32)
            - self.padding.horizontal() as i32
            - scrollbar_w as i32;
        let view_h = cmp::min(viewport.height as i32, layout.bounds().height as i32)
            - self.padding.vertical() as i32;

        if view_w <= 0 || view_h <= 0 {
            // Zero sized image
            return;
        }

        let mut terminal = self.terminal.lock().unwrap();

        // Ensure terminal is the right size
        terminal.resize(view_w as u32, view_h as u32);

        // Update if needed
        if terminal.needs_update {
            terminal.update();
            terminal.needs_update = false;
        }

        // Render default background
        {
            let meta = &terminal.metadata_set[terminal.default_attrs().metadata];
            let background_color = shade(meta.bg, state.is_focused);

            renderer.fill_quad(
                Quad {
                    bounds: layout.bounds(),
                    border: self.border,
                    ..Default::default()
                },
                Color::new(
                    background_color.r() as f32 / 255.0,
                    background_color.g() as f32 / 255.0,
                    background_color.b() as f32 / 255.0,
                    match self.opacity {
                        Some(opacity) => opacity,
                        None => background_color.a() as f32 / 255.0,
                    },
                ),
            );
        }

        // Render cell backgrounds that do not match default
        terminal.with_buffer(|buffer| {
            for run in buffer.layout_runs() {
                struct BgRect<'a> {
                    default_metadata: usize,
                    metadata: usize,
                    glyph_font_size: f32,
                    start_x: f32,
                    end_x: f32,
                    line_height: f32,
                    line_top: f32,
                    view_position: Point,
                    metadata_set: &'a IndexSet<Metadata>,
                }

                impl<'a> BgRect<'a> {
                    fn update<Renderer: renderer::Renderer>(
                        &mut self,
                        glyph: &LayoutGlyph,
                        renderer: &mut Renderer,
                        is_focused: bool,
                    ) {
                        if glyph.metadata == self.metadata {
                            self.end_x = glyph.x + glyph.w;
                        } else {
                            self.fill(renderer, is_focused);
                            self.metadata = glyph.metadata;
                            self.glyph_font_size = glyph.font_size;
                            self.start_x = glyph.x;
                            self.end_x = glyph.x + glyph.w;
                        }
                    }

                    fn fill<Renderer: renderer::Renderer>(
                        &mut self,
                        renderer: &mut Renderer,
                        is_focused: bool,
                    ) {
                        let cosmic_text_to_iced_color = |color: cosmic_text::Color| {
                            Color::new(
                                color.r() as f32 / 255.0,
                                color.g() as f32 / 255.0,
                                color.b() as f32 / 255.0,
                                color.a() as f32 / 255.0,
                            )
                        };

                        macro_rules! mk_pos_offset {
                            ($x_offset:expr, $bottom_offset:expr) => {
                                Vector::new(
                                    self.start_x + $x_offset,
                                    self.line_top + self.line_height - $bottom_offset,
                                )
                            };
                        }

                        macro_rules! mk_quad {
                            ($pos_offset:expr, $style_line_height:expr, $width:expr) => {
                                Quad {
                                    bounds: Rectangle::new(
                                        self.view_position + $pos_offset,
                                        Size::new($width, $style_line_height),
                                    ),
                                    ..Default::default()
                                }
                            };
                            ($pos_offset:expr, $style_line_height:expr) => {
                                mk_quad!($pos_offset, $style_line_height, self.end_x - self.start_x)
                            };
                        }

                        let metadata = &self.metadata_set[self.metadata];
                        if metadata.bg != self.metadata_set[self.default_metadata].bg {
                            let color = shade(metadata.bg, is_focused);
                            renderer.fill_quad(
                                mk_quad!(mk_pos_offset!(0.0, self.line_height), self.line_height),
                                cosmic_text_to_iced_color(color),
                            );
                        }

                        if !metadata.flags.is_empty() {
                            let style_line_height = (self.glyph_font_size / 10.0).clamp(2.0, 16.0);

                            let line_color = cosmic_text_to_iced_color(metadata.underline_color);

                            if metadata.flags.contains(Flags::STRIKEOUT) {
                                let bottom_offset = (self.line_height - style_line_height) / 2.0;
                                let pos_offset = mk_pos_offset!(0.0, bottom_offset);
                                let underline_quad = mk_quad!(pos_offset, style_line_height);
                                renderer.fill_quad(underline_quad, line_color);
                            }

                            if metadata.flags.contains(Flags::UNDERLINE) {
                                let bottom_offset = style_line_height * 2.0;
                                let pos_offset = mk_pos_offset!(0.0, bottom_offset);
                                let underline_quad = mk_quad!(pos_offset, style_line_height);
                                renderer.fill_quad(underline_quad, line_color);
                            }

                            if metadata.flags.contains(Flags::DOUBLE_UNDERLINE) {
                                let style_line_height = style_line_height / 2.0;
                                let gap = style_line_height.max(1.5);
                                let bottom_offset = (style_line_height + gap) * 2.0;

                                let pos_offset1 = mk_pos_offset!(0.0, bottom_offset);
                                let underline1_quad = mk_quad!(pos_offset1, style_line_height);

                                let pos_offset2 = mk_pos_offset!(0.0, bottom_offset / 2.0);
                                let underline2_quad = mk_quad!(pos_offset2, style_line_height);

                                renderer.fill_quad(underline1_quad, line_color);
                                renderer.fill_quad(underline2_quad, line_color);
                            }

                            if metadata.flags.contains(Flags::DOTTED_UNDERLINE) {
                                let bottom_offset = style_line_height * 2.0;

                                let full_width = self.end_x - self.start_x;
                                let mut accu_width = 0.0;
                                let mut dot_width = 2.0f32.min(full_width - accu_width);

                                while accu_width < full_width {
                                    dot_width = dot_width.min(full_width - accu_width);
                                    let pos_offset = mk_pos_offset!(accu_width, bottom_offset);
                                    let underline_quad =
                                        mk_quad!(pos_offset, style_line_height, dot_width);
                                    renderer.fill_quad(underline_quad, line_color);
                                    accu_width += 2.0 * dot_width;
                                }
                            }

                            if metadata.flags.contains(Flags::DASHED_UNDERLINE) {
                                let bottom_offset = style_line_height * 2.0;

                                let full_width = self.end_x - self.start_x;
                                let mut accu_width = 0.0;
                                let mut dash_width = 6.0f32.min(full_width - accu_width);
                                let gap_width = dash_width / 2.0;

                                // gap-width dash first
                                let pos_offset = mk_pos_offset!(accu_width, bottom_offset);
                                let underline_quad =
                                    mk_quad!(pos_offset, style_line_height, gap_width);
                                renderer.fill_quad(underline_quad, line_color);
                                accu_width += gap_width * 2.0;

                                while accu_width < full_width {
                                    dash_width = dash_width.min(full_width - accu_width);
                                    let pos_offset = mk_pos_offset!(accu_width, bottom_offset);
                                    let underline_quad =
                                        mk_quad!(pos_offset, style_line_height, dash_width);
                                    renderer.fill_quad(underline_quad, line_color);
                                    accu_width += dash_width + gap_width;
                                }
                            }

                            if metadata.flags.contains(Flags::UNDERCURL) {
                                let style_line_height = style_line_height.floor();
                                let bottom_offset = style_line_height * 1.5;

                                let full_width = self.end_x - self.start_x;
                                let mut accu_width = 0.0;
                                let mut dot_width = 1.0f32.min(full_width - accu_width);

                                while accu_width < full_width {
                                    dot_width = dot_width.min(full_width - accu_width);

                                    let dot_bottom_offset = match accu_width as u32 % 8 {
                                        3..=5 => bottom_offset + style_line_height,
                                        2 | 6 => bottom_offset + 2.0 * style_line_height / 3.0,
                                        1 | 7 => bottom_offset + 1.0 * style_line_height / 3.0,
                                        _ => bottom_offset,
                                    };

                                    let pos_offset = mk_pos_offset!(accu_width, dot_bottom_offset);
                                    let underline_quad =
                                        mk_quad!(pos_offset, style_line_height, dot_width);
                                    renderer.fill_quad(underline_quad, line_color);
                                    accu_width += dot_width;
                                }
                            }
                        }
                    }
                }

                let default_metadata = terminal.default_attrs().metadata;
                let metadata_set = &terminal.metadata_set;
                let mut bg_rect = BgRect {
                    default_metadata,
                    metadata: default_metadata,
                    glyph_font_size: 0.0,
                    start_x: 0.0,
                    end_x: 0.0,
                    line_height: buffer.metrics().line_height,
                    line_top: run.line_top,
                    view_position,
                    metadata_set,
                };
                for glyph in run.glyphs {
                    bg_rect.update(glyph, renderer, state.is_focused);
                }
                bg_rect.fill(renderer, state.is_focused);
            }
        });

        renderer.fill_raw(Raw {
            buffer: terminal.buffer_weak(),
            position: view_position,
            color: Color::new(1.0, 1.0, 1.0, 1.0), // TODO
            clip_bounds: Rectangle::new(view_position, Size::new(view_w as f32, view_h as f32)),
        });

        // Draw scrollbar
        if let Some((start, end)) = terminal.scrollbar() {
            let scrollbar_y = start * view_h as f32;
            let scrollbar_h = end * view_h as f32 - scrollbar_y;
            let scrollbar_rect = Rectangle::new(
                [view_w as f32, scrollbar_y].into(),
                Size::new(scrollbar_w, scrollbar_h),
            );

            let pressed = matches!(&state.dragging, Some(Dragging::Scrollbar { .. }));

            let mut hover = false;
            if let Some(p) = cursor_position.position_in(layout.bounds()) {
                let x = p.x - self.padding.left;
                if x >= scrollbar_rect.x && x < (scrollbar_rect.x + scrollbar_rect.width) {
                    hover = true;
                }
            }

            let mut scrollbar_draw = scrollbar_rect + Vector::new(view_position.x, view_position.y);
            if !hover && !pressed {
                // Decrease draw width and keep centered when not hovered or pressed
                scrollbar_draw.width /= 2.0;
                scrollbar_draw.x += scrollbar_draw.width / 2.0;
            }

            // neutral_6, 0.7
            let base_color = cosmic_theme
                .palette
                .neutral_6
                .without_alpha()
                .with_alpha(0.7);
            let scrollbar_color: Color = if pressed {
                // pressed_state_color, 0.5
                cosmic_theme
                    .background
                    .component
                    .pressed
                    .without_alpha()
                    .with_alpha(0.5)
                    .over(base_color)
                    .into()
            } else if hover {
                // hover_state_color, 0.2
                cosmic_theme
                    .background
                    .component
                    .hover
                    .without_alpha()
                    .with_alpha(0.2)
                    .over(base_color)
                    .into()
            } else {
                base_color.into()
            };

            renderer.fill_quad(
                Quad {
                    bounds: scrollbar_draw,
                    border: Border {
                        radius: (scrollbar_draw.width / 2.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                scrollbar_color,
            );

            state.scrollbar_rect.set(scrollbar_rect);
        } else {
            state.scrollbar_rect.set(Rectangle::default())
        }

        let duration = instant.elapsed();
        log::trace!("redraw {}, {}: {:?}", view_w, view_h, duration);
    }

    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor_position: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle<f32>,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();
        let scrollbar_rect = state.scrollbar_rect.get();
        let mut terminal = self.terminal.lock().unwrap();
        let buffer_size = terminal.with_buffer(|buffer| buffer.size());

        let is_app_cursor = terminal.term.lock().mode().contains(TermMode::APP_CURSOR);
        let is_mouse_mode = terminal.term.lock().mode().intersects(TermMode::MOUSE_MODE);

        let mut status = Status::Ignored;
        match event {
            Event::Keyboard(KeyEvent::KeyPressed {
                key: Key::Named(named),
                modifiers,
                ..
            }) if state.is_focused => {
                for (key_bind, _) in &self.key_binds {
                    if key_bind.matches(modifiers, &Key::Named(named)) {
                        return Status::Captured;
                    }
                }
                let mod_no = calculate_modifier_number(state);
                let escape_code = match named {
                    Named::Insert => csi("2", "~", mod_no),
                    Named::Delete => csi("3", "~", mod_no),
                    Named::PageUp => {
                        if modifiers.shift() {
                            terminal.scroll(TerminalScroll::PageUp);
                            None
                        } else {
                            csi("5", "~", mod_no)
                        }
                    }
                    Named::PageDown => {
                        if modifiers.shift() {
                            terminal.scroll(TerminalScroll::PageDown);
                            None
                        } else {
                            csi("6", "~", mod_no)
                        }
                    }
                    Named::ArrowUp => {
                        if is_app_cursor {
                            ss3("A", mod_no)
                        } else {
                            csi("A", "", mod_no)
                        }
                    }
                    Named::ArrowDown => {
                        if is_app_cursor {
                            ss3("B", mod_no)
                        } else {
                            csi("B", "", mod_no)
                        }
                    }
                    Named::ArrowRight => {
                        if is_app_cursor {
                            ss3("C", mod_no)
                        } else {
                            csi("C", "", mod_no)
                        }
                    }
                    Named::ArrowLeft => {
                        if is_app_cursor {
                            ss3("D", mod_no)
                        } else {
                            csi("D", "", mod_no)
                        }
                    }
                    Named::End => {
                        if modifiers.shift() {
                            terminal.scroll(TerminalScroll::Bottom);
                            None
                        } else if is_app_cursor {
                            ss3("F", mod_no)
                        } else {
                            csi("F", "", mod_no)
                        }
                    }
                    Named::Home => {
                        if modifiers.shift() {
                            terminal.scroll(TerminalScroll::Top);
                            None
                        } else if is_app_cursor {
                            ss3("H", mod_no)
                        } else {
                            csi("H", "", mod_no)
                        }
                    }
                    Named::F1 => ss3("P", mod_no),
                    Named::F2 => ss3("Q", mod_no),
                    Named::F3 => ss3("R", mod_no),
                    Named::F4 => ss3("S", mod_no),
                    Named::F5 => csi("15", "~", mod_no),
                    Named::F6 => csi("17", "~", mod_no),
                    Named::F7 => csi("18", "~", mod_no),
                    Named::F8 => csi("19", "~", mod_no),
                    Named::F9 => csi("20", "~", mod_no),
                    Named::F10 => csi("21", "~", mod_no),
                    Named::F11 => csi("23", "~", mod_no),
                    Named::F12 => csi("24", "~", mod_no),
                    _ => None,
                };
                if let Some(escape_code) = escape_code {
                    terminal.input_scroll(escape_code);
                    return Status::Captured;
                }

                //Special handle Enter, Escape, Backspace and Tab as described in
                //https://sw.kovidgoyal.net/kitty/keyboard-protocol/#legacy-key-event-encoding
                //Also special handle Ctrl-_ to behave like xterm
                let alt_prefix = if modifiers.alt() { "\x1B" } else { "" };
                match named {
                    Named::Backspace => {
                        let code = if modifiers.control() { "\x08" } else { "\x7f" };
                        terminal.input_scroll(format!("{alt_prefix}{code}").as_bytes().to_vec());
                        status = Status::Captured;
                    }
                    Named::Enter => {
                        terminal
                            .input_scroll(format!("{}{}", alt_prefix, "\x0D").as_bytes().to_vec());
                        status = Status::Captured;
                    }
                    Named::Escape => {
                        //Escape with any modifier will cancel selection
                        let had_selection = {
                            let mut term = terminal.term.lock();
                            term.selection.take().is_some()
                        };
                        if had_selection {
                            terminal.update();
                        } else {
                            terminal.input_scroll(
                                format!("{}{}", alt_prefix, "\x1B").as_bytes().to_vec(),
                            );
                        }
                        status = Status::Captured;
                    }
                    Named::Space => {
                        terminal.input_scroll(format!("{}{}", alt_prefix, " ").as_bytes().to_vec());
                        status = Status::Captured;
                    }
                    Named::Tab => {
                        let code = if modifiers.shift() { "\x1b[Z" } else { "\x09" };
                        terminal.input_scroll(format!("{alt_prefix}{code}").as_bytes().to_vec());
                        status = Status::Captured;
                    }
                    _ => {}
                }
            }
            Event::Keyboard(KeyEvent::ModifiersChanged(modifiers)) => {
                state.modifiers = modifiers;
            }
            Event::Keyboard(KeyEvent::KeyPressed {
                text,
                modifiers,
                key,
                ..
            }) if state.is_focused => {
                for (key_bind, _) in &self.key_binds {
                    if key_bind.matches(modifiers, &key) {
                        return Status::Captured;
                    }
                }
                let character = text.and_then(|c| c.chars().next()).unwrap_or_default();
                match (
                    modifiers.logo(),
                    modifiers.control(),
                    modifiers.alt(),
                    modifiers.shift(),
                ) {
                    (true, _, _, _) => {
                        // Ignore super
                    }
                    (false, true, true, _) => {
                        // Handle ctrl-alt for non-control characters
                        // and control characters 0-32
                        if !character.is_control() || (character as u32) < 32 {
                            // Handle alt for non-control characters
                            let mut buf = [0x1B, 0, 0, 0, 0];
                            let len = {
                                let str = character.encode_utf8(&mut buf[1..]);
                                str.len() + 1
                            };
                            terminal.input_scroll(buf[..len].to_vec());
                            status = Status::Captured;
                        }
                    }
                    (false, true, _, false) => {
                        // Handle ctrl for control characters (Ctrl-A to Ctrl-Z)
                        if character.is_control() {
                            let mut buf = [0, 0, 0, 0];
                            let str = character.encode_utf8(&mut buf);
                            terminal.input_scroll(str.as_bytes().to_vec());
                            status = Status::Captured;
                        }
                    }
                    (false, true, _, true) => {
                        //This is normally Ctrl+Minus, but since that
                        //is taken by zoom, we send that code for
                        //Ctrl+Underline instead, like xterm and
                        //gnome-terminal
                        if key == Key::Character("_".into()) {
                            terminal.input_scroll(b"\x1F".as_slice());
                            status = Status::Captured;
                        }
                    }
                    (false, false, true, _) => {
                        if !character.is_control() {
                            // Handle alt for non-control characters
                            let mut buf = [0x1B, 0, 0, 0, 0];
                            let len = {
                                let str = character.encode_utf8(&mut buf[1..]);
                                str.len() + 1
                            };
                            terminal.input_scroll(buf[..len].to_vec());
                            status = Status::Captured;
                        }
                    }
                    (false, false, false, _) => {
                        // Handle no modifiers for non-control characters
                        if !character.is_control() {
                            let mut buf = [0, 0, 0, 0];
                            let str = character.encode_utf8(&mut buf);
                            terminal.input_scroll(str.as_bytes().to_vec());
                            status = Status::Captured;
                        }
                    }
                }
            }
            Event::Mouse(MouseEvent::ButtonPressed(button)) => {
                if let Some(p) = cursor_position.position_in(layout.bounds()) {
                    let x = p.x - self.padding.left;
                    let y = p.y - self.padding.top;
                    //TODO: better calculation of position
                    let col = x / terminal.size().cell_width;
                    let row = y / terminal.size().cell_height;

                    if is_mouse_mode {
                        terminal.report_mouse(event, &state.modifiers, col as u32, row as u32);
                    } else {
                        state.is_focused = true;

                        // Handle left click drag
                        #[allow(clippy::collapsible_if)]
                        if let Button::Left = button {
                            let x = p.x - self.padding.left;
                            let y = p.y - self.padding.top;
                            if x >= 0.0 && x < buffer_size.0 && y >= 0.0 && y < buffer_size.1 {
                                let click_kind =
                                    if let Some((click_kind, click_time)) = state.click.take() {
                                        if click_time.elapsed() < self.click_timing {
                                            match click_kind {
                                                ClickKind::Single => ClickKind::Double,
                                                ClickKind::Double => ClickKind::Triple,
                                                ClickKind::Triple => ClickKind::Single,
                                            }
                                        } else {
                                            ClickKind::Single
                                        }
                                    } else {
                                        ClickKind::Single
                                    };
                                let location = terminal.viewport_to_point(TermPoint::new(
                                    row as usize,
                                    TermColumn(col as usize),
                                ));
                                let side = if col.fract() < 0.5 {
                                    TermSide::Left
                                } else {
                                    TermSide::Right
                                };
                                let selection = match click_kind {
                                    ClickKind::Single => {
                                        Selection::new(SelectionType::Simple, location, side)
                                    }
                                    ClickKind::Double => {
                                        Selection::new(SelectionType::Semantic, location, side)
                                    }
                                    ClickKind::Triple => {
                                        Selection::new(SelectionType::Lines, location, side)
                                    }
                                };
                                {
                                    let mut term = terminal.term.lock();
                                    term.selection = Some(selection);
                                }
                                terminal.needs_update = true;
                                state.click = Some((click_kind, Instant::now()));
                                state.dragging = Some(Dragging::Buffer);
                            } else if scrollbar_rect.contains(Point::new(x, y)) {
                                if let Some(start_scroll) = terminal.scrollbar() {
                                    state.dragging = Some(Dragging::Scrollbar {
                                        start_y: y,
                                        start_scroll,
                                    });
                                }
                            } else if x >= scrollbar_rect.x
                                && x < (scrollbar_rect.x + scrollbar_rect.width)
                            {
                                if terminal.scrollbar().is_some() {
                                    let scroll_ratio =
                                        terminal.with_buffer(|buffer| y / buffer.size().1);
                                    terminal.scroll_to(scroll_ratio);
                                    if let Some(start_scroll) = terminal.scrollbar() {
                                        state.dragging = Some(Dragging::Scrollbar {
                                            start_y: y,
                                            start_scroll,
                                        });
                                    }
                                }
                            }
                        }
                        // Update context menu state
                        if let Some(on_context_menu) = &self.on_context_menu {
                            shell.publish((on_context_menu)(match self.context_menu {
                                Some(_) => None,
                                None => match button {
                                    Button::Right => Some(p),
                                    _ => None,
                                },
                            }));
                        }
                        status = Status::Captured;
                    }
                }
            }
            Event::Mouse(MouseEvent::ButtonReleased(Button::Left)) => {
                state.dragging = None;
                if let Some(p) = cursor_position.position_in(layout.bounds()) {
                    let x = p.x - self.padding.left;
                    let y = p.y - self.padding.top;
                    //TODO: better calculation of position
                    let col = x / terminal.size().cell_width;
                    let row = y / terminal.size().cell_height;
                    if is_mouse_mode {
                        terminal.report_mouse(event, &state.modifiers, col as u32, row as u32);
                    } else {
                        status = Status::Captured;
                    }
                } else {
                    status = Status::Captured;
                }
            }
            Event::Mouse(MouseEvent::ButtonReleased(_button)) => {
                if let Some(p) = cursor_position.position_in(layout.bounds()) {
                    let x = p.x - self.padding.left;
                    let y = p.y - self.padding.top;
                    //TODO: better calculation of position
                    let col = x / terminal.size().cell_width;
                    let row = y / terminal.size().cell_height;
                    if is_mouse_mode {
                        terminal.report_mouse(event, &state.modifiers, col as u32, row as u32);
                    }
                }
            }
            Event::Mouse(MouseEvent::CursorMoved { .. }) => {
                if let Some(on_mouse_enter) = &self.on_mouse_enter {
                    let mouse_is_inside = cursor_position.position_in(layout.bounds()).is_some();
                    if let Some(known_is_inside) = self.mouse_inside_boundary {
                        if mouse_is_inside != known_is_inside {
                            if mouse_is_inside {
                                shell.publish(on_mouse_enter());
                            }
                            self.mouse_inside_boundary = Some(mouse_is_inside);
                        }
                    } else {
                        self.mouse_inside_boundary = Some(mouse_is_inside);
                    }
                }
                if let Some(p) = cursor_position.position() {
                    let x = (p.x - layout.bounds().x) - self.padding.left;
                    let y = (p.y - layout.bounds().y) - self.padding.top;
                    //TODO: better calculation of position
                    let col = x / terminal.size().cell_width;
                    let row = y / terminal.size().cell_height;
                    if is_mouse_mode {
                        terminal.report_mouse(event, &state.modifiers, col as u32, row as u32);
                    } else {
                        if let Some(dragging) = &state.dragging {
                            match dragging {
                                Dragging::Buffer => {
                                    let location = terminal.viewport_to_point(TermPoint::new(
                                        row as usize,
                                        TermColumn(col as usize),
                                    ));
                                    let side = if col.fract() < 0.5 {
                                        TermSide::Left
                                    } else {
                                        TermSide::Right
                                    };
                                    {
                                        let mut term = terminal.term.lock();
                                        if let Some(selection) = &mut term.selection {
                                            selection.update(location, side);
                                        }
                                    }
                                    terminal.needs_update = true;
                                }
                                Dragging::Scrollbar {
                                    start_y,
                                    start_scroll,
                                } => {
                                    let scroll_offset = terminal
                                        .with_buffer(|buffer| ((y - start_y) / buffer.size().1));
                                    terminal.scroll_to(start_scroll.0 + scroll_offset);
                                }
                            }
                        }
                        status = Status::Captured;
                    }
                }
            }
            Event::Mouse(MouseEvent::WheelScrolled { delta }) => {
                if let Some(p) = cursor_position.position_in(layout.bounds()) {
                    if is_mouse_mode {
                        let x = (p.x - layout.bounds().x) - self.padding.left;
                        let y = (p.y - layout.bounds().y) - self.padding.top;
                        //TODO: better calculation of position
                        let col = x / terminal.size().cell_width;
                        let row = y / terminal.size().cell_height;
                        terminal.scroll_mouse(delta, &state.modifiers, col as u32, row as u32);
                    } else {
                        match delta {
                            ScrollDelta::Lines { x: _, y } => {
                                //TODO: this adjustment is just a guess!
                                state.scroll_pixels = 0.0;
                                let lines = (-y * 6.0) as i32;
                                if lines != 0 {
                                    terminal.scroll(TerminalScroll::Delta(-lines));
                                }
                                status = Status::Captured;
                            }
                            ScrollDelta::Pixels { x: _, y } => {
                                //TODO: this adjustment is just a guess!
                                state.scroll_pixels -= y * 6.0;
                                let mut lines = 0;
                                let metrics = terminal.with_buffer(|buffer| buffer.metrics());
                                while state.scroll_pixels <= -metrics.line_height {
                                    lines -= 1;
                                    state.scroll_pixels += metrics.line_height;
                                }
                                while state.scroll_pixels >= metrics.line_height {
                                    lines += 1;
                                    state.scroll_pixels -= metrics.line_height;
                                }
                                if lines != 0 {
                                    terminal.scroll(TerminalScroll::Delta(-lines));
                                }
                                status = Status::Captured;
                            }
                        }
                    }
                }
            }
            _ => (),
        }

        status
    }
}

fn shade(color: cosmic_text::Color, is_focused: bool) -> cosmic_text::Color {
    if is_focused {
        color
    } else {
        let shade = 0.92;
        cosmic_text::Color::rgba(
            (color.r() as f32 * shade) as u8,
            (color.g() as f32 * shade) as u8,
            (color.b() as f32 * shade) as u8,
            color.a(),
        )
    }
}

impl<'a, Message> From<TerminalBox<'a, Message>> for Element<'a, Message, cosmic::Theme, Renderer>
where
    Message: Clone + 'a,
{
    fn from(terminal_box: TerminalBox<'a, Message>) -> Self {
        Self::new(terminal_box)
    }
}

enum ClickKind {
    Single,
    Double,
    Triple,
}

enum Dragging {
    Buffer,
    Scrollbar {
        start_y: f32,
        start_scroll: (f32, f32),
    },
}

pub struct State {
    modifiers: Modifiers,
    click: Option<(ClickKind, Instant)>,
    dragging: Option<Dragging>,
    is_focused: bool,
    scroll_pixels: f32,
    scrollbar_rect: Cell<Rectangle<f32>>,
}

impl State {
    /// Creates a new [`State`].
    pub fn new() -> State {
        State {
            modifiers: Modifiers::empty(),
            click: None,
            dragging: None,
            is_focused: false,
            scroll_pixels: 0.0,
            scrollbar_rect: Cell::new(Rectangle::default()),
        }
    }
}

impl operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.is_focused
    }

    fn focus(&mut self) {
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
    }
}

/*
 shift     0b1         (1)
alt       0b10        (2)
ctrl      0b100       (4)
super     0b1000      (8)
hyper     0b10000     (16)
meta      0b100000    (32)
caps_lock 0b1000000   (64)
num_lock  0b10000000  (128)
*/
fn calculate_modifier_number(state: &State) -> u8 {
    let mut mod_no = 0;
    if state.modifiers.shift() {
        mod_no |= 1;
    }
    if state.modifiers.alt() {
        mod_no |= 2;
    }
    if state.modifiers.control() {
        mod_no |= 4;
    }
    if state.modifiers.logo() {
        mod_no |= 8;
    }
    mod_no + 1
}

#[inline(always)]
fn csi(code: &str, suffix: &str, modifiers: u8) -> Option<Vec<u8>> {
    if modifiers == 1 {
        Some(format!("\x1B[{code}{suffix}").as_bytes().to_vec())
    } else {
        Some(
            format!("\x1B[{code};{modifiers}{suffix}")
                .as_bytes()
                .to_vec(),
        )
    }
}

#[inline(always)]
fn ss3(code: &str, modifiers: u8) -> Option<Vec<u8>> {
    if modifiers == 1 {
        Some(format!("\x1B\x4F{code}").as_bytes().to_vec())
    } else {
        Some(format!("\x1B[1;{modifiers}{code}").as_bytes().to_vec())
    }
}
