use cosmic_text::{Buffer, FontSystem, SwashCache, SwashContent};
use std::{rc::Rc, slice};
use tiny_skia::{ColorU8, Paint, Pixmap, PixmapPaint, PixmapRef, Rect, Transform};
use winit::window::Window;

pub struct SoftwareRenderer {
    pub window: Rc<Window>,
    pub context: softbuffer::GraphicsContext,
}

impl SoftwareRenderer {
    pub fn new(window: Rc<Window>) -> Result<Self, String> {
        let context = unsafe { softbuffer::GraphicsContext::new(&*window, &*window) }
            .map_err(|err| format!("failed to create context: {}", err))?;
        Ok(Self { window, context })
    }

    pub fn render(
        &mut self,
        buffer: &mut Buffer,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) {
        let (width, height) = {
            let size = self.window.inner_size();
            (size.width, size.height)
        };

        let mut pixmap = Pixmap::new(width, height).unwrap();
        //TODO: configurable background
        pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0xFF));

        let line_height = buffer.metrics().line_height;
        let mut paint = Paint::default();
        paint.anti_alias = false;
        let pixmap_paint = PixmapPaint::default();
        let transform = Transform::identity();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0., 0.), 1.0);

                let glyph_color = match glyph.color_opt {
                    Some(some) => some,
                    None => cosmic_text::Color::rgb(0xFF, 0xFF, 0xFF),
                };

                let background_color = cosmic_text::Color(glyph.metadata as u32);
                if background_color.0 != 0xFF000000 {
                    //TODO: Have to swap RGB for BGR
                    paint.set_color_rgba8(
                        background_color.b(),
                        background_color.g(),
                        background_color.r(),
                        background_color.a(),
                    );
                    pixmap.fill_rect(
                        Rect::from_xywh(glyph.x, run.line_top, glyph.w, line_height).unwrap(),
                        &paint,
                        transform,
                        None,
                    );
                }

                match swash_cache.get_image(font_system, physical_glyph.cache_key) {
                    Some(image) if !image.data.is_empty() => {
                        let mut data = Vec::with_capacity(
                            (image.placement.width * image.placement.height) as usize,
                        );
                        match image.content {
                            SwashContent::Mask => {
                                let mut i = 0;
                                while i < image.data.len() {
                                    //TODO: Have to swap RGB for BGR
                                    data.push(
                                        ColorU8::from_rgba(
                                            glyph_color.b(),
                                            glyph_color.g(),
                                            glyph_color.r(),
                                            image.data[i],
                                        )
                                        .premultiply(),
                                    );
                                    i += 1;
                                }
                            }
                            SwashContent::Color => {
                                let mut i = 0;
                                while i < image.data.len() {
                                    //TODO: Have to swap RGB for BGR
                                    data.push(
                                        ColorU8::from_rgba(
                                            image.data[i + 2],
                                            image.data[i + 1],
                                            image.data[i],
                                            image.data[i + 3],
                                        )
                                        .premultiply(),
                                    );
                                    i += 4;
                                }
                            }
                            SwashContent::SubpixelMask => {
                                todo!("TODO: SubpixelMask");
                            }
                        }

                        let glyph_pixmap = PixmapRef::from_bytes(
                            unsafe {
                                slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4)
                            },
                            image.placement.width,
                            image.placement.height,
                        )
                        .unwrap();
                        pixmap.draw_pixmap(
                            physical_glyph.x + image.placement.left,
                            run.line_y as i32 + physical_glyph.y - image.placement.top,
                            glyph_pixmap,
                            &pixmap_paint,
                            transform,
                            None,
                        );
                    }
                    _ => {}
                }
            }
        }
        buffer.set_redraw(false);

        self.context.set_buffer(
            bytemuck::cast_slice(pixmap.data()),
            width as u16,
            height as u16,
        );
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        //TODO: resize image here for better performance
        self.window.request_redraw();
    }
}
