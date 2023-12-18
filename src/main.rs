use alacritty_terminal::{
    ansi::{Color, NamedColor},
    config::{Config, PtyConfig},
    event::{Event as TermEvent, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop as PtyEventLoop, Notifier},
    grid::Dimensions,
    index::{Column, Line, Point},
    sync::FairMutex,
    term::{
        cell::Flags,
        color::{Colors, Rgb},
    },
    tty, Term,
};
use cosmic_text::{
    Attrs, AttrsList, Buffer, BufferLine, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    Weight, Wrap,
};
use std::{num::NonZeroU32, rc::Rc, slice, sync::Arc};
use tiny_skia::{Paint, PixmapMut, Rect, Transform};
use winit::{
    event::{ElementState, Event as WinitEvent, KeyEvent, Modifiers, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    keyboard::ModifiersState,
    window::WindowBuilder,
};

#[derive(Clone, Copy, Debug)]
struct Size {
    width: f32,
    height: f32,
    cell_width: f32,
    cell_height: f32,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        (self.height / self.cell_height).floor() as usize
    }

    fn columns(&self) -> usize {
        (self.width / self.cell_width).floor() as usize
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
struct EventProxy(EventLoopProxy<TermEvent>);

impl EventListener for EventProxy {
    fn send_event(&self, event: TermEvent) {
        let _ = self.0.send_event(event);
    }
}

fn colors() -> Colors {
    let mut colors = Colors::default();

    // These colors come from `ransid`: https://gitlab.redox-os.org/redox-os/ransid/-/blob/master/src/color.rs
    let encode_rgb = |r: u8, g: u8, b: u8| -> Rgb { Rgb { r, g, b } };
    for value in 0..=255 {
        let color = match value {
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
    colors[NamedColor::Foreground] = colors[NamedColor::White];
    colors[NamedColor::Background] = colors[NamedColor::Black];
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

fn main() {
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    let metrics = Metrics::new(14.0, 20.0);
    //TODO: set color to default fg
    let default_attrs = Attrs::new().family(Family::Monospace);
    let mut buffer = Buffer::new_empty(metrics);
    buffer.set_wrap(&mut font_system, Wrap::None);
    let (cell_width, cell_height) = {
        // Use size of space to determine cell size
        buffer.set_text(&mut font_system, " ", default_attrs, Shaping::Advanced);
        let layout = buffer.line_layout(&mut font_system, 0).unwrap();
        (layout[0].w, metrics.line_height)
    };
    println!("{}, {}", cell_width, cell_height);

    let event_loop = EventLoopBuilder::<TermEvent>::with_user_event()
        .build()
        .unwrap();
    let event_loop_proxy = event_loop.create_proxy();
    let window = Rc::new(WindowBuilder::new().build(&event_loop).unwrap());
    let (width, height) = {
        let inner_size = window.inner_size();
        (inner_size.width as f32, inner_size.height as f32)
    };

    let config = Config::default();
    let mut dimensions = Size {
        width,
        height,
        cell_width,
        cell_height,
    };
    let event_proxy = EventProxy(event_loop_proxy);
    let term = Arc::new(FairMutex::new(Term::new(
        &config,
        &dimensions,
        event_proxy.clone(),
    )));
    let colors = colors();

    let pty_config = PtyConfig::default();
    let window_id = 0;
    let pty = tty::new(&pty_config, dimensions.into(), window_id).unwrap();

    let pty_event_loop = PtyEventLoop::new(term.clone(), event_proxy, pty, pty_config.hold, false);
    let mut notifier = Notifier(pty_event_loop.channel());
    let pty_join_handle = pty_event_loop.spawn();

    let context = softbuffer::Context::new(window.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
    let mut modifiers = Modifiers::default();
    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            match event {
                WinitEvent::WindowEvent {
                    window_id,
                    event: WindowEvent::RedrawRequested,
                } if window_id == window.id() => {
                    let (width, height) = {
                        let size = window.inner_size();
                        (size.width, size.height)
                    };
                    surface
                        .resize(
                            NonZeroU32::new(width).unwrap(),
                            NonZeroU32::new(height).unwrap(),
                        )
                        .unwrap();

                    let mut surface_buffer = surface.buffer_mut().unwrap();
                    let surface_buffer_u8 = unsafe {
                        slice::from_raw_parts_mut(
                            surface_buffer.as_mut_ptr() as *mut u8,
                            surface_buffer.len() * 4,
                        )
                    };
                    let mut pixmap =
                        PixmapMut::from_bytes(surface_buffer_u8, width, height).unwrap();
                    pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0xFF));

                    // Set scroll to view scroll
                    //TODO buffer.set_scroll(*scroll);
                    // Set size, will relayout and shape until scroll if changed
                    buffer.set_size(&mut font_system, width as f32, height as f32);
                    // Shape until scroll, ensures scroll is clamped
                    buffer.shape_until_scroll(&mut font_system, true);
                    // Update scroll after buffer clamps it
                    //TODO *scroll = buffer.scroll();

                    let mut paint = Paint::default();
                    paint.anti_alias = false;
                    let transform = Transform::identity();
                    let mut f = |x: f32, y: f32, w: f32, h: f32, color: cosmic_text::Color| {
                        // Have to swap RGB for BGR
                        paint.set_color_rgba8(color.b(), color.g(), color.r(), color.a());
                        pixmap.fill_rect(
                            Rect::from_xywh(x, y, w, h).unwrap(),
                            &paint,
                            transform,
                            None,
                        );
                    };
                    for run in buffer.layout_runs() {
                        for glyph in run.glyphs.iter() {
                            let physical_glyph = glyph.physical((0., 0.), 1.0);

                            let glyph_color = match glyph.color_opt {
                                Some(some) => some,
                                None => cosmic_text::Color::rgb(0xFF, 0xFF, 0xFF),
                            };

                            let background_color = cosmic_text::Color(glyph.metadata as u32);

                            f(
                                glyph.x,
                                run.line_top,
                                glyph.w,
                                metrics.line_height,
                                background_color,
                            );

                            swash_cache.with_pixels(
                                &mut font_system,
                                physical_glyph.cache_key,
                                glyph_color,
                                |x, y, color| {
                                    f(
                                        (physical_glyph.x + x) as f32,
                                        run.line_y + (physical_glyph.y + y) as f32,
                                        1.0,
                                        1.0,
                                        color,
                                    );
                                },
                            );
                        }
                    }
                    buffer.set_redraw(false);

                    surface_buffer.present().unwrap();
                }
                WinitEvent::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    text: Some(text),
                                    state: ElementState::Pressed,
                                    ..
                                },
                            ..
                        },
                    window_id,
                } if window_id == window.id() => {
                    println!("{:?} {:?}", modifiers, text);
                    match (
                        modifiers.state().contains(ModifiersState::SUPER),
                        modifiers.state().contains(ModifiersState::CONTROL),
                        modifiers.state().contains(ModifiersState::ALT),
                    ) {
                        (true, _, _) => {} // Ignore super
                        (false, true, _) => {
                            // Control keys
                            if text.len() == 1 {
                                let c = text.chars().next().unwrap_or_default();
                                if c >= 'a' && c <= 'z' {
                                    notifier.notify(vec![(c as u8) - b'a' + 1]);
                                }
                            }
                        }
                        (false, false, true) => {
                            // Alt keys
                            let mut bytes = text.as_bytes().to_vec();
                            bytes.insert(0, b'\x1B');
                            notifier.notify(bytes);
                        }
                        (false, false, false) => {
                            notifier.notify(text.as_bytes().to_vec());
                        }
                    }
                }
                WinitEvent::WindowEvent {
                    event: WindowEvent::ModifiersChanged(new_modifiers),
                    window_id,
                } if window_id == window.id() => {
                    modifiers = new_modifiers;
                }
                WinitEvent::WindowEvent {
                    event: WindowEvent::Resized(physical_size),
                    window_id,
                } if window_id == window.id() => {
                    dimensions.width = physical_size.width as f32;
                    dimensions.height = physical_size.height as f32;
                    notifier.on_resize(dimensions.into());
                    term.lock().resize(dimensions);
                }
                WinitEvent::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == window.id() => {
                    term.lock().exit();
                }
                WinitEvent::UserEvent(user_event) => {
                    println!("{:?}", user_event);
                    match user_event {
                        TermEvent::PtyWrite(text) => notifier.notify(text.into_bytes()),
                        TermEvent::Exit => elwt.exit(),
                        _ => {}
                    }

                    //TODO: is redraw needed after all events?
                    //TODO: use LineDamageBounds
                    {
                        let mut last_point = Point::new(Line(0), Column(0));
                        let mut text = String::new();
                        let mut attrs_list = AttrsList::new(default_attrs);
                        for indexed in term.lock().grid().display_iter() {
                            if indexed.point.line != last_point.line {
                                let line_i = last_point.line.0 as usize;
                                while line_i >= buffer.lines.len() {
                                    buffer.lines.push(BufferLine::new(
                                        "",
                                        AttrsList::new(default_attrs),
                                        Shaping::Advanced,
                                    ));
                                    buffer.set_redraw(true);
                                }

                                if buffer.lines[line_i].set_text(text.clone(), attrs_list.clone()) {
                                    buffer.set_redraw(true);
                                }

                                text.clear();
                                attrs_list.clear_spans();
                            }
                            //TODO: use indexed.point.column?

                            let start = text.len();
                            text.push(indexed.cell.c);
                            let end = text.len();

                            let convert_color = |color| {
                                let rgb = match color {
                                    Color::Named(named_color) => match colors[named_color] {
                                        Some(rgb) => rgb,
                                        None => {
                                            eprintln!("missing named color {:?}", named_color);
                                            Rgb::default()
                                        }
                                    },
                                    Color::Spec(rgb) => rgb,
                                    Color::Indexed(index) => match colors[index as usize] {
                                        Some(rgb) => rgb,
                                        None => {
                                            eprintln!("missing indexed color {}", index);
                                            Rgb::default()
                                        }
                                    },
                                };
                                cosmic_text::Color::rgb(rgb.r, rgb.g, rgb.b)
                            };

                            let mut attrs = default_attrs;
                            attrs = attrs.color(convert_color(indexed.cell.fg));
                            // Use metadata as background color
                            attrs = attrs.metadata(convert_color(indexed.cell.bg).0 as usize);
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

                            last_point = indexed.point;
                        }

                        //TODO: do not repeat!
                        let line_i = last_point.line.0 as usize;
                        while line_i >= buffer.lines.len() {
                            buffer.lines.push(BufferLine::new(
                                "",
                                AttrsList::new(default_attrs),
                                Shaping::Advanced,
                            ));
                            buffer.set_redraw(true);
                        }

                        if buffer.lines[line_i].set_text(text, attrs_list) {
                            buffer.set_redraw(true);
                        }
                    }

                    if buffer.redraw() {
                        window.request_redraw();
                    }
                }
                _ => {}
            }
        })
        .unwrap();

    //TODO: hangs after event loop exit pty_join_handle.join().unwrap();
}
