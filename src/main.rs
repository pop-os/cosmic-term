use alacritty_terminal::{
    config::{Config, PtyConfig},
    event::{Event as TermEvent, EventListener, Notify, WindowSize},
    event_loop::{EventLoop as PtyEventLoop, Notifier},
    grid::Dimensions,
    index::{Column, Line, Point},
    sync::FairMutex,
    term::cell::Flags,
    tty, Term,
};
use cosmic_text::{
    Attrs, AttrsList, Buffer, BufferLine, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    Weight,
};
use std::{num::NonZeroU32, rc::Rc, slice, sync::Arc};
use tiny_skia::{Color, Paint, PixmapMut, Rect, Transform};
use winit::{
    event::{ElementState, Event as WinitEvent, KeyEvent, Modifiers, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::WindowBuilder,
};

struct TermSize {
    rows: usize,
    cols: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

#[derive(Clone)]
struct EventProxy(EventLoopProxy<TermEvent>);

impl EventListener for EventProxy {
    fn send_event(&self, event: TermEvent) {
        let _ = self.0.send_event(event);
    }
}

fn main() {
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    let metrics = Metrics::new(14.0, 20.0);
    let mut buffer = Buffer::new_empty(metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);

    let event_loop = EventLoopBuilder::<TermEvent>::with_user_event()
        .build()
        .unwrap();
    let event_loop_proxy = event_loop.create_proxy();
    let window = Rc::new(WindowBuilder::new().build(&event_loop).unwrap());

    let config = Config::default();
    let dimensions = TermSize { rows: 24, cols: 80 };
    let event_proxy = EventProxy(event_loop_proxy);
    let term = Arc::new(FairMutex::new(Term::new(
        &config,
        &dimensions,
        event_proxy.clone(),
    )));

    let pty_config = PtyConfig::default();
    let window_size = WindowSize {
        num_lines: dimensions.rows as u16,
        num_cols: dimensions.cols as u16,
        cell_width: 8,   /*TODO*/
        cell_height: 16, /*TODO*/
    };
    let window_id = 0;
    let pty = tty::new(&pty_config, window_size, window_id).unwrap();

    let pty_event_loop = PtyEventLoop::new(term.clone(), event_proxy, pty, pty_config.hold, false);
    let notifier = Notifier(pty_event_loop.channel());
    let pty_join_handle = pty_event_loop.spawn();

    for i in 0..dimensions.rows {
        notifier.notify(format!("echo {}\r", i).into_bytes());
    }

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
                    pixmap.fill(Color::from_rgba8(0, 0, 0, 0xFF));

                    // Set scroll to view scroll
                    //TODO buffer.set_scroll(*scroll);
                    // Set size, will relayout and shape until scroll if changed
                    buffer.set_size(width as f32, height as f32);
                    // Shape until scroll, ensures scroll is clamped
                    buffer.shape_until_scroll(true);
                    // Update scroll after buffer clamps it
                    //TODO *scroll = buffer.scroll();

                    let mut paint = Paint::default();
                    paint.anti_alias = false;
                    let transform = Transform::identity();
                    buffer.draw(
                        &mut swash_cache,
                        cosmic_text::Color::rgb(0xFF, 0xFF, 0xFF),
                        |x, y, w, h, color| {
                            paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
                            pixmap.fill_rect(
                                Rect::from_xywh(x as f32, y as f32, w as f32, h as f32).unwrap(),
                                &paint,
                                transform,
                                None,
                            );
                        },
                    );
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
                    println!("{:?}", text);
                    notifier.notify(text.as_bytes().to_vec());
                }
                WinitEvent::WindowEvent {
                    event: WindowEvent::ModifiersChanged(new_modifiers),
                    window_id,
                } if window_id == window.id() => {
                    modifiers = new_modifiers;
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
                        //TODO: set color to default fg
                        let default_attrs = Attrs::new().family(Family::Monospace);
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

                            let mut attrs = default_attrs;
                            println!("{:?}", indexed.cell.fg);
                            //TODO: fg and bg
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
