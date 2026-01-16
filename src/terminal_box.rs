// SPDX-License-Identifier: GPL-3.0-only

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column as TermColumn, Point as TermPoint, Side as TermSide},
    selection::{Selection, SelectionType},
    term::{TermMode, cell::Flags},
    vte::ansi::{CursorShape, NamedColor},
};
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::{
    Renderer,
    cosmic_theme::palette::{WithAlpha, blend::Compose},
    iced::{
        Color, Element, Length, Padding, Point, Rectangle, Size, Vector,
        advanced::graphics::text::Raw,
        event::{Event, Status},
        keyboard::{Event as KeyEvent, Key, Modifiers},
        mouse::{self, Button, Event as MouseEvent, ScrollDelta},
        window::RedrawRequest,
    },
    iced_core::{
        Border, Shell,
        clipboard::Clipboard,
        keyboard::key::Named,
        layout::{self, Layout},
        renderer::{self, Quad, Renderer as _},
        text::Renderer as _,
        widget::{
            self, Id, Widget,
            operation::{self, Operation},
            tree,
        },
    },
    theme::Theme,
};
use cosmic_text::LayoutGlyph;
use indexmap::IndexSet;
use std::{
    array,
    cell::Cell,
    cmp,
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::{
    Action, Terminal, TerminalScroll, key_bind::key_binds, menu::MenuState,
    mouse_reporter::MouseReporter, terminal::Metadata,
};

const AUTOSCROLL_INTERVAL: Duration = Duration::from_millis(100);

/// Drives repeated drag updates while the pointer is outside the widget.
struct DragAutoscroll {
    active: bool,
    pointer: Option<Point>,
    last_tick: Instant,
    interval: Duration,
}

impl DragAutoscroll {
    fn new(interval: Duration) -> Self {
        Self {
            active: false,
            pointer: None,
            last_tick: Instant::now(),
            interval,
        }
    }

    fn start(&mut self, pointer: Point) {
        self.pointer = Some(pointer);
        if !self.active {
            self.active = true;
            self.last_tick = Instant::now();
        }
    }

    fn update_pointer(&mut self, pointer: Point) {
        if self.active {
            self.pointer = Some(pointer);
        }
    }

    fn stop(&mut self) {
        self.active = false;
        self.pointer = None;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the stored pointer when the next tick is due.
    fn next_due(&mut self) -> Option<(Point, f32)> {
        if self.active && self.last_tick.elapsed() >= self.interval {
            let elapsed = self.last_tick.elapsed();
            self.last_tick = Instant::now();
            let ticks = (elapsed.as_secs_f32() / self.interval.as_secs_f32()).max(1.0);
            self.pointer.map(|pointer| (pointer, ticks))
        } else {
            None
        }
    }
}

pub struct TerminalBox<'a, Message> {
    terminal: &'a Mutex<Terminal>,
    id: Option<Id>,
    border: Border,
    padding: Padding,
    show_headerbar: bool,
    click_timing: Duration,
    context_menu: Option<Point>,
    on_context_menu: Option<Box<dyn Fn(Option<MenuState>) -> Message + 'a>>,
    on_mouse_enter: Option<Box<dyn Fn() -> Message + 'a>>,
    opacity: Option<f32>,
    mouse_inside_boundary: Option<bool>,
    on_middle_click: Option<Box<dyn Fn() -> Message + 'a>>,
    on_open_hyperlink: Option<Box<dyn Fn(String) -> Message + 'a>>,
    on_window_focused: Option<Box<dyn Fn() -> Message + 'a>>,
    on_window_unfocused: Option<Box<dyn Fn() -> Message + 'a>>,
    key_binds: HashMap<KeyBind, Action>,
    sharp_corners: bool,
    disabled: bool,
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
            show_headerbar: true,
            click_timing: Duration::from_millis(500),
            context_menu: None,
            on_context_menu: None,
            on_mouse_enter: None,
            opacity: None,
            mouse_inside_boundary: None,
            on_middle_click: None,
            key_binds: key_binds(),
            on_open_hyperlink: None,
            on_window_focused: None,
            on_window_unfocused: None,
            sharp_corners: false,
            disabled: false,
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

    pub fn show_headerbar(mut self, show_headerbar: bool) -> Self {
        self.show_headerbar = show_headerbar;
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
        on_context_menu: impl Fn(Option<MenuState>) -> Message + 'a,
    ) -> Self {
        self.on_context_menu = Some(Box::new(on_context_menu));
        self
    }

    pub fn on_mouse_enter(mut self, on_mouse_enter: impl Fn() -> Message + 'a) -> Self {
        self.on_mouse_enter = Some(Box::new(on_mouse_enter));
        self
    }

    pub fn on_middle_click(mut self, on_middle_click: impl Fn() -> Message + 'a) -> Self {
        self.on_middle_click = Some(Box::new(on_middle_click));
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity);
        self
    }

    pub fn sharp_corners(mut self, sharp_corners: bool) -> Self {
        self.sharp_corners = sharp_corners;
        self
    }

    pub fn on_open_hyperlink(
        mut self,
        on_open_hyperlink: Option<Box<dyn Fn(String) -> Message + 'a>>,
    ) -> Self {
        self.on_open_hyperlink = on_open_hyperlink;
        self
    }

    pub fn on_window_focused(mut self, on_window_focused: impl Fn() -> Message + 'a) -> Self {
        self.on_window_focused = Some(Box::new(on_window_focused));
        self
    }

    pub fn on_window_unfocused(mut self, on_window_unfocused: impl Fn() -> Message + 'a) -> Self {
        self.on_window_unfocused = Some(Box::new(on_window_unfocused));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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
                if let Some(layout) = line.layout_opt() {
                    layout_lines += layout.len()
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
        operation: &mut dyn Operation,
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
            if x >= 0.0
                && x < buffer_size.0.unwrap_or(0.0)
                && y >= 0.0
                && y < buffer_size.1.unwrap_or(0.0)
            {
                if state.modifiers.contains(Modifiers::CTRL) {
                    let col = x / terminal.size().cell_width;
                    let row = y / terminal.size().cell_height;

                    let location = terminal
                        .viewport_to_point(TermPoint::new(row as usize, TermColumn(col as usize)));
                    if get_hyperlink(&terminal, location).is_some() {
                        return mouse::Interaction::Pointer;
                    }
                }

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
        // matches the corners to the window border
        let corner_radius = if self.sharp_corners {
            cosmic_theme.radius_0()
        } else {
            cosmic_theme.radius_s()
        }
        .map(|x| if x < 4.0 { x - 1.0 } else { x + 3.0 });
        let scrollbar_w = f32::from(cosmic_theme.spacing.space_xxs);

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
            let background_color = shade(meta.bg, state.is_focused && !self.disabled);

            renderer.fill_quad(
                Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        radius: if self.show_headerbar {
                            [0.0, 0.0, corner_radius[2], corner_radius[3]].into()
                        } else {
                            corner_radius.into()
                        },
                        width: self.border.width,
                        color: self.border.color,
                    },
                    ..Default::default()
                },
                Color::new(
                    f32::from(background_color.r()) / 255.0,
                    f32::from(background_color.g()) / 255.0,
                    f32::from(background_color.b()) / 255.0,
                    match self.opacity {
                        Some(opacity) => opacity,
                        None => f32::from(background_color.a()) / 255.0,
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
                                f32::from(color.r()) / 255.0,
                                f32::from(color.g()) / 255.0,
                                f32::from(color.b()) / 255.0,
                                f32::from(color.a()) / 255.0,
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
                            let style_line_height = (self.glyph_font_size / 10.0).clamp(1.0, 16.0);

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

                            // rects is a slice of (width, Option<y>), `None` means a gap.
                            let mut draw_repeated = |rects: &[(f32, Option<f32>)]| {
                                let full_width = self.end_x - self.start_x;
                                let pattern_len: f32 = rects.iter().map(|x| x.0).sum(); // total length of the pattern
                                let mut accu_width = 0.0;
                                let mut index = {
                                    let in_pattern = self.start_x % pattern_len;

                                    let mut sum = 0.0;
                                    let mut index = 0;
                                    for (i, rect) in rects.iter().enumerate() {
                                        sum += rect.0;
                                        if in_pattern < sum {
                                            let width = sum - in_pattern;
                                            if let Some(height) = rect.1 {
                                                // draw first rect cropped to span
                                                let pos_offset = mk_pos_offset!(accu_width, height);
                                                let underline_quad =
                                                    mk_quad!(pos_offset, style_line_height, width);
                                                renderer.fill_quad(underline_quad, line_color);
                                            }
                                            index = i + 1;
                                            accu_width += width;
                                            break;
                                        }
                                    }
                                    index // index of first full rect
                                };
                                while accu_width < full_width {
                                    let (width, x) = rects[index % rects.len()];
                                    let cropped_width = width.min(full_width - accu_width);
                                    if let Some(height) = x {
                                        let pos_offset = mk_pos_offset!(accu_width, height);
                                        let underline_quad =
                                            mk_quad!(pos_offset, style_line_height, cropped_width);
                                        renderer.fill_quad(underline_quad, line_color);
                                    }
                                    accu_width += cropped_width;
                                    index += 1;
                                }
                            };

                            if metadata.flags.contains(Flags::DOTTED_UNDERLINE) {
                                let bottom_offset = style_line_height * 2.0;
                                let dot = (2.0, Some(bottom_offset));
                                let gap = (2.0, None);
                                draw_repeated(&[dot, gap]);
                            }

                            if metadata.flags.contains(Flags::DASHED_UNDERLINE) {
                                let bottom_offset = style_line_height * 2.0;
                                let dash = (6.0, Some(bottom_offset));
                                let gap = (3.0, None);
                                draw_repeated(&[dash, gap]);
                            }

                            if metadata.flags.contains(Flags::UNDERCURL) {
                                let style_line_height = style_line_height.floor();
                                let bottom_offset = style_line_height * 1.5;
                                let pattern: [(f32, Option<f32>); 8] =
                                    array::from_fn(|i| match i {
                                        3..=5 => (1.0, Some(bottom_offset + style_line_height)),
                                        2 | 6 => (
                                            1.0,
                                            Some(bottom_offset + 2.0 * style_line_height / 3.0),
                                        ),
                                        1 | 7 => (
                                            1.0,
                                            Some(bottom_offset + 1.0 * style_line_height / 3.0),
                                        ),
                                        0 => (1.0, Some(bottom_offset)),
                                        _ => unreachable!(),
                                    });
                                draw_repeated(&pattern)
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

        // Draw cursor (only when not scrolled, as cursor is at bottom of active area)
        {
            let term = terminal.term.lock();
            let display_offset = term.grid().display_offset();
            let cursor = term.renderable_content().cursor;
            drop(term);

            // Skip drawing cursor when scrolled - the cursor is below the visible viewport
            if display_offset > 0 {
                // Cursor is off-screen when scrolled up
            } else {
                let col = cursor.point.column.0;
                let line = cursor.point.line.0;
                let color = terminal.term.lock().colors()[NamedColor::Cursor]
                    .or(terminal.colors()[NamedColor::Cursor])
                    .map(|rgb| Color::from_rgb8(rgb.r, rgb.g, rgb.b))
                    .unwrap_or(Color::WHITE); // TODO default color from theme?
                let width = terminal.size().cell_width;
                let height = terminal.size().cell_height;
                let top_left = view_position
                    + Vector::new((col as f32 * width).floor(), (line as f32 * height).floor());
                match cursor.shape {
                    CursorShape::Beam => {
                        let quad = Quad {
                            bounds: Rectangle::new(top_left, Size::new(1.0, height)),
                            ..Default::default()
                        };
                        renderer.fill_quad(quad, color);
                    }
                    CursorShape::Underline => {
                        let quad = Quad {
                            bounds: Rectangle::new(
                                view_position
                                    + Vector::new(
                                        (col as f32 * width).floor(),
                                        ((line + 1) as f32 * height).floor(),
                                    ),
                                Size::new(width, 1.0),
                            ),
                            ..Default::default()
                        };
                        renderer.fill_quad(quad, color);
                    }
                    CursorShape::Block if !state.is_focused => {
                        let quad = Quad {
                            bounds: Rectangle::new(top_left, Size::new(width, height)),
                            border: Border {
                                width: 1.0,
                                color,
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        renderer.fill_quad(quad, Color::TRANSPARENT);
                    }
                    CursorShape::HollowBlock => {
                        let quad = Quad {
                            bounds: Rectangle::new(top_left, Size::new(width, height)),
                            border: Border {
                                width: 1.0,
                                color,
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        renderer.fill_quad(quad, Color::TRANSPARENT);
                    }
                    CursorShape::Block | CursorShape::Hidden => {} // Block is handled seperately
                }
            }
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
        if self.disabled {
            return Status::Ignored;
        }
        let state = tree.state.downcast_mut::<State>();
        let scrollbar_rect = state.scrollbar_rect.get();
        let mut terminal = self.terminal.lock().unwrap();
        let buffer_size = terminal.with_buffer(|buffer| buffer.size());

        let is_app_cursor = terminal.term.lock().mode().contains(TermMode::APP_CURSOR);
        let is_mouse_mode = terminal.term.lock().mode().intersects(TermMode::MOUSE_MODE);
        let mut status = Status::Ignored;
        match event {
            Event::Window(event) => match event {
                cosmic::iced::window::Event::Focused => {
                    if let Some(on_window_focused) = &self.on_window_focused {
                        shell.publish(on_window_focused());
                    }
                }
                cosmic::iced::window::Event::RedrawRequested(_) => {
                    if is_mouse_mode {
                        state.autoscroll.stop();
                    } else {
                        if let Some((pointer, multiplier)) = state.autoscroll.next_due() {
                            if update_buffer_drag(
                                state,
                                &mut terminal,
                                buffer_size,
                                pointer,
                                layout.bounds(),
                                self.padding,
                                multiplier,
                            ) {
                                status = Status::Captured;
                            }
                        }
                        if state.autoscroll.is_active() {
                            shell.request_redraw(RedrawRequest::NextFrame);
                        }
                    }
                }
                cosmic::iced::window::Event::Unfocused => {
                    state.is_focused = false;
                    state.autoscroll.stop();
                    if let Some(on_window_unfocused) = &self.on_window_unfocused {
                        shell.publish(on_window_unfocused());
                    }
                }
                _ => {}
            },
            Event::Keyboard(KeyEvent::KeyPressed {
                key: Key::Named(named),
                modified_key: Key::Named(modified_named),
                modifiers,
                text,
                ..
            }) if state.is_focused && named == modified_named => {
                for key_bind in self.key_binds.keys() {
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
                            csi2("A", mod_no)
                        }
                    }
                    Named::ArrowDown => {
                        if is_app_cursor {
                            ss3("B", mod_no)
                        } else {
                            csi2("B", mod_no)
                        }
                    }
                    Named::ArrowRight => {
                        if is_app_cursor {
                            ss3("C", mod_no)
                        } else {
                            csi2("C", mod_no)
                        }
                    }
                    Named::ArrowLeft => {
                        if is_app_cursor {
                            ss3("D", mod_no)
                        } else {
                            csi2("D", mod_no)
                        }
                    }
                    Named::End => {
                        if modifiers.shift() {
                            terminal.scroll(TerminalScroll::Bottom);
                            None
                        } else if is_app_cursor {
                            ss3("F", mod_no)
                        } else {
                            csi2("F", mod_no)
                        }
                    }
                    Named::Home => {
                        if modifiers.shift() {
                            terminal.scroll(TerminalScroll::Top);
                            None
                        } else if is_app_cursor {
                            ss3("H", mod_no)
                        } else {
                            csi2("H", mod_no)
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
                        terminal.input_scroll(format!("{alt_prefix}{code}").into_bytes());
                        status = Status::Captured;
                    }
                    Named::Enter => {
                        terminal.input_scroll(format!("{}{}", alt_prefix, "\x0D").into_bytes());
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
                            terminal.input_scroll(format!("{}{}", alt_prefix, "\x1B").into_bytes());
                        }
                        status = Status::Captured;
                    }
                    Named::Space => {
                        // Keep this instead of hardcoding the space to allow for dead keys
                        let character = text.and_then(|c| c.chars().next()).unwrap_or_default();

                        if modifiers.control() {
                            // Send NUL character (\x00) for Ctrl + Space
                            terminal.input_scroll(b"\x00".to_vec());
                        } else {
                            terminal
                                .input_scroll(format!("{}{}", alt_prefix, character).into_bytes());
                        }
                        status = Status::Captured;
                    }
                    Named::Tab => {
                        let code = if modifiers.shift() { "\x1b[Z" } else { "\x09" };
                        terminal.input_scroll(format!("{alt_prefix}{code}").into_bytes());
                        status = Status::Captured;
                    }
                    _ => {}
                }
            }
            Event::Keyboard(KeyEvent::ModifiersChanged(modifiers)) => {
                state.modifiers = modifiers;

                if modifiers.contains(Modifiers::CTRL)
                    || terminal.active_regex_match.is_some()
                    || terminal.active_hyperlink_id.is_some()
                {
                    //Might need to update the url regex highlight,
                    //so we need to calculate the mouse position
                    let location = if let Some(p) = cursor_position.position_in(layout.bounds()) {
                        let x = p.x - self.padding.left;
                        let y = p.y - self.padding.top;
                        //TODO: better calculation of position
                        let col = x / terminal.size().cell_width;
                        let row = y / terminal.size().cell_height;
                        Some(terminal.viewport_to_point(TermPoint::new(
                            row as usize,
                            TermColumn(col as usize),
                        )))
                    } else {
                        None
                    };
                    update_active_regex_match(&mut terminal, location, Some(&state.modifiers));
                }
            }
            Event::Keyboard(KeyEvent::KeyPressed {
                text,
                modifiers,
                key,
                ..
            }) if state.is_focused => {
                for key_bind in self.key_binds.keys() {
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
                        state.autoscroll.stop();
                        terminal.report_mouse(event, &state.modifiers, col as u32, row as u32);
                    } else {
                        state.is_focused = true;

                        // Handle left click drag
                        #[allow(clippy::collapsible_if)]
                        if let Button::Left = button {
                            let x = p.x - self.padding.left;
                            let y = p.y - self.padding.top;
                            if x >= 0.0
                                && x < buffer_size.0.unwrap_or(0.0)
                                && y >= 0.0
                                && y < buffer_size.1.unwrap_or(0.0)
                            {
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
                                // Check if shift is pressed and there's an existing selection to extend
                                if state.modifiers.shift() {
                                    let mut term = terminal.term.lock();
                                    if let Some(ref mut selection) = term.selection {
                                        selection.update(location, side);
                                    } else {
                                        term.selection = Some(Selection::new(
                                            SelectionType::Simple,
                                            location,
                                            side,
                                        ));
                                    }
                                } else {
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
                                    let mut term = terminal.term.lock();
                                    term.selection = Some(selection);
                                }
                                terminal.needs_update = true;
                                state.click = Some((click_kind, Instant::now()));
                                state.dragging = Some(Dragging::Buffer {
                                    edge_scroll_remainder: 0.0,
                                    last_edge_direction: EdgeScrollDirection::None,
                                    last_edge_overshoot: 0.0,
                                    last_point: location,
                                    last_side: side,
                                });
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
                                    let scroll_ratio = terminal
                                        .with_buffer(|buffer| y / buffer.size().1.unwrap_or(1.0));
                                    terminal.scroll_to(scroll_ratio);
                                    if let Some(start_scroll) = terminal.scrollbar() {
                                        state.dragging = Some(Dragging::Scrollbar {
                                            start_y: y,
                                            start_scroll,
                                        });
                                    }
                                }
                            }
                        } else if button == Button::Middle {
                            if let Some(on_middle_click) = &self.on_middle_click {
                                shell.publish(on_middle_click());
                            }
                        }
                        // Update context menu state
                        if let Some(on_context_menu) = &self.on_context_menu {
                            match self.context_menu {
                                Some(_) => {
                                    shell.publish(on_context_menu(None));
                                }
                                None => {
                                    if button == Button::Right {
                                        let x = p.x - self.padding.left;
                                        let y = p.y - self.padding.top;
                                        //TODO: better calculation of position
                                        let col = x / terminal.size().cell_width;
                                        let row = y / terminal.size().cell_height;

                                        let location = terminal.viewport_to_point(TermPoint::new(
                                            row as usize,
                                            TermColumn(col as usize),
                                        ));
                                        update_active_regex_match(
                                            &mut terminal,
                                            Some(location),
                                            None,
                                        );
                                        let link = get_hyperlink(&terminal, location);
                                        shell.publish(on_context_menu(Some(MenuState {
                                            position: Some(p),
                                            link,
                                        })));
                                    }
                                }
                            }
                        }
                        status = Status::Captured;
                    }
                }
            }
            Event::Mouse(MouseEvent::ButtonReleased(Button::Left)) => {
                state.autoscroll.stop();
                if let Some(dragging) = state.dragging.take() {
                    if let Dragging::Buffer {
                        last_point,
                        last_side,
                        ..
                    } = dragging
                    {
                        {
                            let mut term = terminal.term.lock();
                            if let Some(selection) = &mut term.selection {
                                selection.update(last_point, last_side);
                            }
                        }
                        terminal.needs_update = true;
                    }
                }
                if let Some(p) = cursor_position.position_in(layout.bounds()) {
                    let x = p.x - self.padding.left;
                    let y = p.y - self.padding.top;
                    //TODO: better calculation of position
                    let col = x / terminal.size().cell_width;
                    let row = y / terminal.size().cell_height;

                    let location = terminal
                        .viewport_to_point(TermPoint::new(row as usize, TermColumn(col as usize)));
                    if state.modifiers.control() {
                        if let Some(on_open_hyperlink) = &self.on_open_hyperlink {
                            if let Some(hyperlink) = get_hyperlink(&terminal, location) {
                                shell.publish(on_open_hyperlink(hyperlink));
                                status = Status::Captured;
                            }
                        }
                    }

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
                state.autoscroll.stop();
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
                if let Some(p_global) = cursor_position.position() {
                    let bounds = layout.bounds();
                    let col_row_opt = if let Some(p) = cursor_position.position_in(bounds) {
                        let x = p.x - self.padding.left;
                        let y = p.y - self.padding.top;
                        //TODO: better calculation of position
                        let col = x / terminal.size().cell_width;
                        let row = y / terminal.size().cell_height;
                        let location = terminal.viewport_to_point(TermPoint::new(
                            row as usize,
                            TermColumn(col as usize),
                        ));
                        update_active_regex_match(
                            &mut terminal,
                            Some(location),
                            Some(&state.modifiers),
                        );
                        Some((col, row))
                    } else {
                        None
                    };

                    if is_mouse_mode {
                        if let Some((col, row)) = col_row_opt {
                            terminal.report_mouse(event, &state.modifiers, col as u32, row as u32);
                        }
                    } else {
                        let handled_buffer_drag = update_buffer_drag(
                            state,
                            &mut terminal,
                            buffer_size,
                            p_global,
                            bounds,
                            self.padding,
                            0.0,
                        );
                        if handled_buffer_drag {
                            status = Status::Captured;
                        } else if let Some(Dragging::Scrollbar {
                            start_y,
                            start_scroll,
                        }) = state.dragging.as_mut()
                        {
                            let y = p_global.y - bounds.y - self.padding.top;
                            let start_y = *start_y;
                            let start_scroll = *start_scroll;
                            let scroll_offset = terminal.with_buffer(|buffer| {
                                (y - start_y) / buffer.size().1.unwrap_or(1.0)
                            });
                            terminal.scroll_to(start_scroll.0 + scroll_offset);
                            status = Status::Captured;
                        }

                        if matches!(state.dragging, Some(Dragging::Buffer { .. })) {
                            if cursor_position.position_in(bounds).is_some() {
                                state.autoscroll.stop();
                            } else {
                                if state.autoscroll.is_active() {
                                    state.autoscroll.update_pointer(p_global);
                                } else {
                                    state.autoscroll.start(p_global);
                                }
                                shell.request_redraw(RedrawRequest::NextFrame);
                            }
                        } else {
                            state.autoscroll.stop();
                        }
                    }
                }
            }
            Event::Mouse(MouseEvent::WheelScrolled { delta }) => {
                if let Some(p) = cursor_position.position_in(layout.bounds()) {
                    if is_mouse_mode {
                        let x = p.x - self.padding.left;
                        let y = p.y - self.padding.top;
                        //TODO: better calculation of position
                        let col = x / terminal.size().cell_width;
                        let row = y / terminal.size().cell_height;
                        terminal.scroll_mouse(delta, &state.modifiers, col as u32, row as u32);
                    } else if terminal.term.lock().mode().contains(TermMode::ALT_SCREEN) {
                        MouseReporter::report_mouse_wheel_as_arrows(
                            &terminal,
                            terminal.size().cell_width,
                            terminal.size().cell_height,
                            delta,
                        );
                        status = Status::Captured;
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
                    {
                        let x = p.x - self.padding.left;
                        let y = p.y - self.padding.top;
                        //TODO: better calculation of position
                        let col = x / terminal.size().cell_width;
                        let row = y / terminal.size().cell_height;

                        let location = terminal.viewport_to_point(TermPoint::new(
                            row as usize,
                            TermColumn(col as usize),
                        ));
                        update_active_regex_match(
                            &mut terminal,
                            Some(location),
                            Some(&state.modifiers),
                        );
                    }
                }
            }
            _ => (),
        }

        status
    }
}

fn get_hyperlink(
    terminal: &std::sync::MutexGuard<'_, Terminal>,
    location: TermPoint,
) -> Option<String> {
    if let Some(link) = osc8_hyperlink_at(terminal, location) {
        return Some(link.uri().to_string());
    }
    if let Some(match_) = terminal
        .regex_matches
        .iter()
        .find(|bounds| bounds.contains(&location))
    {
        let term = terminal.term.lock();
        Some(term.bounds_to_string(*match_.start(), *match_.end()))
    } else {
        None
    }
}

fn get_hyperlink_id(
    terminal: &std::sync::MutexGuard<'_, Terminal>,
    location: TermPoint,
) -> Option<String> {
    osc8_hyperlink_at(terminal, location).map(|link| link.id().to_string())
}

fn osc8_hyperlink_at(
    terminal: &std::sync::MutexGuard<'_, Terminal>,
    location: TermPoint,
) -> Option<alacritty_terminal::term::cell::Hyperlink> {
    let term = terminal.term.lock();
    if location.line >= term.screen_lines() || location.column.0 >= term.columns() {
        return None;
    }
    let grid = term.grid();
    let cell = &grid[location];
    if let Some(link) = cell.hyperlink() {
        return Some(link);
    }
    if cell
        .flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        && location.column.0 > 0
    {
        let left = TermPoint::new(location.line, TermColumn(location.column.0 - 1));
        return grid[left].hyperlink();
    }
    None
}

fn update_active_regex_match(
    terminal: &mut std::sync::MutexGuard<'_, Terminal>,
    location: Option<TermPoint>,
    modifiers: Option<&Modifiers>,
) {
    //Do not update any highlights if
    //there is a context_menu shown
    //to the user
    if terminal.context_menu.is_some() {
        return;
    }

    let allow_hyperlink = modifiers
        .map(|mods| mods.contains(Modifiers::CTRL))
        .unwrap_or(false);
    //Require CTRL for keyboard and mouse interaction
    if let Some(modifiers) = modifiers {
        if !modifiers.contains(Modifiers::CTRL) {
            if terminal.active_regex_match.is_some() {
                terminal.active_regex_match = None;
                terminal.needs_update = true;
            }
            if terminal.active_hyperlink_id.is_some() {
                terminal.active_hyperlink_id = None;
                terminal.needs_update = true;
            }
            return;
        }
    } else if terminal.active_hyperlink_id.is_some() {
        terminal.active_hyperlink_id = None;
        terminal.needs_update = true;
    }
    let Some(location) = location else {
        if terminal.active_regex_match.is_some() {
            terminal.active_regex_match = None;
            terminal.needs_update = true;
        }
        if terminal.active_hyperlink_id.is_some() {
            terminal.active_hyperlink_id = None;
            terminal.needs_update = true;
        }
        return;
    };
    if allow_hyperlink {
        let next_hyperlink_id = get_hyperlink_id(terminal, location);
        if terminal.active_hyperlink_id != next_hyperlink_id {
            terminal.active_hyperlink_id = next_hyperlink_id;
            terminal.needs_update = true;
        }
    } else if terminal.active_hyperlink_id.is_some() {
        terminal.active_hyperlink_id = None;
        terminal.needs_update = true;
    }
    if let Some(match_) = terminal
        .regex_matches
        .iter()
        .find(|bounds| bounds.contains(&location))
    {
        'update: {
            if let Some(active_match) = &terminal.active_regex_match {
                if active_match == match_ {
                    break 'update;
                }
            }
            terminal.active_regex_match = Some(match_.clone());
            terminal.needs_update = true;
        }
    } else if terminal.active_regex_match.is_some() {
        terminal.active_regex_match = None;
        terminal.needs_update = true;
    }
}

fn shade(color: cosmic_text::Color, is_focused: bool) -> cosmic_text::Color {
    if is_focused {
        color
    } else {
        let shade = 0.92;
        cosmic_text::Color::rgba(
            (f32::from(color.r()) * shade) as u8,
            (f32::from(color.g()) * shade) as u8,
            (f32::from(color.b()) * shade) as u8,
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
    Buffer {
        edge_scroll_remainder: f32,
        last_edge_direction: EdgeScrollDirection,
        last_edge_overshoot: f32,
        last_point: TermPoint,
        last_side: TermSide,
    },
    Scrollbar {
        start_y: f32,
        start_scroll: (f32, f32),
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EdgeScrollDirection {
    None,
    Top,
    Bottom,
}

fn edge_scroll_adjustment(
    y: f32,
    buffer_height: f32,
    cell_height: f32,
    max_row_index: f32,
    scroll_remainder: f32,
    last_direction: EdgeScrollDirection,
    last_overshoot: f32,
    forced_increment: f32,
) -> (i32, f32, f32, EdgeScrollDirection, f32) {
    if cell_height <= 0.0 {
        return (0, 0.0, 0.0, EdgeScrollDirection::None, 0.0);
    }

    let mut row = y / cell_height;
    let mut delta = 0;
    let mut remainder = scroll_remainder;
    let mut direction = EdgeScrollDirection::None;
    let mut overshoot = 0.0;

    if y < 0.0 {
        direction = EdgeScrollDirection::Top;
        overshoot = (-y) / cell_height;
        let previous_overshoot = if last_direction == direction {
            last_overshoot
        } else {
            0.0
        };
        let overshoot_delta = overshoot - previous_overshoot;
        let forced = if forced_increment > 0.0 {
            forced_increment * overshoot.max(1.0)
        } else {
            0.0
        };
        if overshoot_delta > 0.0 || forced > 0.0 {
            remainder = remainder.max(0.0) + overshoot_delta.max(0.0) + forced;
        } else {
            remainder = remainder.max(0.0).min(overshoot.fract());
        }
        delta = remainder.trunc() as i32;
        remainder -= delta as f32;
        row = 0.0;
    } else if y > buffer_height {
        direction = EdgeScrollDirection::Bottom;
        overshoot = (y - buffer_height) / cell_height;
        let previous_overshoot = if last_direction == direction {
            last_overshoot
        } else {
            0.0
        };
        let overshoot_delta = overshoot - previous_overshoot;
        let forced = if forced_increment > 0.0 {
            forced_increment * overshoot.max(1.0)
        } else {
            0.0
        };
        if overshoot_delta > 0.0 || forced > 0.0 {
            remainder = remainder.max(0.0) + overshoot_delta.max(0.0) + forced;
        } else {
            remainder = remainder.max(0.0).min(overshoot.fract());
        }
        let lines = remainder.trunc() as i32;
        delta = -lines;
        remainder -= lines as f32;
        row = max_row_index;
    } else {
        remainder = 0.0;
    }

    row = row.clamp(0.0, max_row_index);

    (delta, row, remainder, direction, overshoot)
}

fn update_buffer_drag(
    state: &mut State,
    terminal: &mut Terminal,
    buffer_size: (Option<f32>, Option<f32>),
    pointer: Point,
    bounds: Rectangle<f32>,
    padding: Padding,
    forced_increment: f32,
) -> bool {
    let Some(dragging) = state.dragging.as_mut() else {
        return false;
    };

    let Dragging::Buffer {
        edge_scroll_remainder,
        last_edge_direction,
        last_edge_overshoot,
        last_point,
        last_side,
    } = dragging
    else {
        return false;
    };

    let x = (pointer.x - bounds.x) - padding.left;
    let y = (pointer.y - bounds.y) - padding.top;

    let size = terminal.size();
    let buffer_height = buffer_size.1.unwrap_or(size.height as f32);
    let max_row_index = (size.screen_lines().saturating_sub(1)) as f32;
    let max_col_index = (size.columns().saturating_sub(1)) as f32;
    let (scroll_delta, adjusted_row, new_remainder, new_direction, new_overshoot) =
        edge_scroll_adjustment(
            y,
            buffer_height,
            size.cell_height,
            max_row_index,
            *edge_scroll_remainder,
            *last_edge_direction,
            *last_edge_overshoot,
            forced_increment,
        );
    *edge_scroll_remainder = new_remainder;
    *last_edge_direction = new_direction;
    *last_edge_overshoot = new_overshoot;
    let col = (x / size.cell_width).clamp(0.0, max_col_index);
    let row = adjusted_row.clamp(0.0, max_row_index);
    if scroll_delta != 0 {
        terminal.scroll(TerminalScroll::Delta(scroll_delta));
    }
    let location =
        terminal.viewport_to_point(TermPoint::new(row as usize, TermColumn(col as usize)));
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
    *last_point = location;
    *last_side = side;
    terminal.needs_update = true;

    true
}

pub struct State {
    modifiers: Modifiers,
    click: Option<(ClickKind, Instant)>,
    dragging: Option<Dragging>,
    is_focused: bool,
    scroll_pixels: f32,
    scrollbar_rect: Cell<Rectangle<f32>>,
    autoscroll: DragAutoscroll,
}

impl State {
    /// Creates a new [`State`].
    pub fn new() -> Self {
        Self {
            modifiers: Modifiers::empty(),
            click: None,
            dragging: None,
            is_focused: false,
            scroll_pixels: 0.0,
            scrollbar_rect: Cell::new(Rectangle::default()),
            autoscroll: DragAutoscroll::new(AUTOSCROLL_INTERVAL),
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
        Some(format!("\x1B[{code}{suffix}").into_bytes())
    } else {
        Some(format!("\x1B[{code};{modifiers}{suffix}").into_bytes())
    }
}
// https://sw.kovidgoyal.net/kitty/keyboard-protocol/#legacy-functional-keys
// CSI 1 ; modifier {ABCDEFHPQS}
// code is ABCDEFHPQS
#[inline(always)]
fn csi2(code: &str, modifiers: u8) -> Option<Vec<u8>> {
    if modifiers == 1 {
        Some(format!("\x1B[{code}").into_bytes())
    } else {
        Some(format!("\x1B[1;{modifiers}{code}").into_bytes())
    }
}

#[inline(always)]
fn ss3(code: &str, modifiers: u8) -> Option<Vec<u8>> {
    if modifiers == 1 {
        Some(format!("\x1B\x4F{code}").into_bytes())
    } else {
        Some(format!("\x1B[1;{modifiers}{code}").into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::{EdgeScrollDirection, edge_scroll_adjustment};

    const BUFFER_HEIGHT: f32 = 200.0;
    const CELL_HEIGHT: f32 = 20.0;
    const MAX_ROW: f32 = 9.0;

    #[test]
    fn edge_scroll_small_top_overshoot_does_not_scroll_immediately() {
        let (delta, row, remainder, direction, overshoot) = edge_scroll_adjustment(
            -5.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            0.0,
            EdgeScrollDirection::None,
            0.0,
            0.0,
        );
        assert_eq!(delta, 0);
        assert_eq!(row, 0.0);
        assert!((remainder - 0.25).abs() < f32::EPSILON);

        let (delta_repeat, row_repeat, remainder_repeat, _, _) = edge_scroll_adjustment(
            -5.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            0.0,
        );
        assert_eq!(delta_repeat, 0);
        assert_eq!(row_repeat, 0.0);
        assert!((remainder_repeat - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn edge_scroll_top_accumulates_into_scroll() {
        let (_delta, _row, mut remainder, mut direction, mut overshoot) = edge_scroll_adjustment(
            -5.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            0.0,
            EdgeScrollDirection::None,
            0.0,
            0.0,
        );
        let (delta, row, new_remainder, new_direction, new_overshoot) = edge_scroll_adjustment(
            -45.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            0.0,
        );
        assert_eq!(delta, 2);
        assert_eq!(row, 0.0);
        assert!((new_remainder - 0.25).abs() < f32::EPSILON);
        remainder = new_remainder;
        direction = new_direction;
        overshoot = new_overshoot;

        // repeated event with the same overshoot should not accumulate more scroll
        let (delta, _, remainder, _, _) = edge_scroll_adjustment(
            -45.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            0.0,
        );
        assert_eq!(delta, 0);
        assert!((remainder - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn edge_scroll_inside_viewport_resets_remainder() {
        let (_delta, _row, remainder, direction, overshoot) = edge_scroll_adjustment(
            -25.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            0.0,
            EdgeScrollDirection::None,
            0.0,
            0.0,
        );
        let (delta, row, remainder, direction, overshoot) = edge_scroll_adjustment(
            60.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            0.0,
        );
        assert_eq!(delta, 0);
        assert!((row - 3.0).abs() < f32::EPSILON);
        assert_eq!(remainder, 0.0);
        assert_eq!(direction, EdgeScrollDirection::None);
        assert_eq!(overshoot, 0.0);
    }

    #[test]
    fn edge_scroll_bottom_accumulates_scroll() {
        let (delta, row, remainder, direction, overshoot) = edge_scroll_adjustment(
            BUFFER_HEIGHT + 1.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            0.0,
            EdgeScrollDirection::None,
            0.0,
            0.0,
        );
        assert_eq!(delta, 0);
        assert!((row - MAX_ROW).abs() < f32::EPSILON);
        assert!((remainder - 0.05).abs() < f32::EPSILON);

        let (delta, row, remainder, direction, overshoot) = edge_scroll_adjustment(
            BUFFER_HEIGHT + 45.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            0.0,
        );
        assert_eq!(delta, -2);
        assert!((row - MAX_ROW).abs() < f32::EPSILON);
        assert!((remainder - 0.25).abs() < f32::EPSILON);

        let (delta, _, remainder, _, _) = edge_scroll_adjustment(
            BUFFER_HEIGHT + 45.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            0.0,
        );
        assert_eq!(delta, 0);
        assert!((remainder - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn edge_scroll_forced_increment_continues_scrolling() {
        let (delta, _, remainder, direction, overshoot) = edge_scroll_adjustment(
            -5.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            0.0,
            EdgeScrollDirection::None,
            0.0,
            0.0,
        );
        assert_eq!(delta, 0);

        let (delta, _, remainder, direction, overshoot) = edge_scroll_adjustment(
            -5.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            1.0,
        );
        assert_eq!(delta, 1);
        assert!((remainder - 0.25).abs() < f32::EPSILON);

        let (delta, _, remainder, _, _) = edge_scroll_adjustment(
            -5.0,
            BUFFER_HEIGHT,
            CELL_HEIGHT,
            MAX_ROW,
            remainder,
            direction,
            overshoot,
            1.0,
        );
        assert_eq!(delta, 1);
        assert!((remainder - 0.25).abs() < f32::EPSILON);
    }
}
