use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer,
};
use std::{error::Error, rc::Rc};
use wgpu::{
    CommandEncoderDescriptor, CompositeAlphaMode, Device, DeviceDescriptor, Features, Instance,
    InstanceDescriptor, Limits, LoadOp, MultisampleState, Operations, PresentMode, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, Surface,
    SurfaceConfiguration, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use winit::window::Window;

pub struct WgpuRenderer {
    pub window: Rc<Window>,
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface,
    pub config: SurfaceConfiguration,
    pub atlas: TextAtlas,
    pub text_renderer: TextRenderer,
}

impl WgpuRenderer {
    async fn new_async(window: Rc<Window>) -> Result<Self, String> {
        let size = window.inner_size();

        let instance = Instance::new(InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .ok_or(format!("failed to request adapter"))?;
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    features: Features::empty(),
                    limits: Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .map_err(|err| format!("failed to request device: {}", err))?;
        let surface = unsafe { instance.create_surface(&*window) }
            .map_err(|err| format!("failed to create surface: {}", err))?;
        let swapchain_format = TextureFormat::Bgra8UnormSrgb;
        let mut config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let mut atlas = TextAtlas::new(&device, &queue, swapchain_format);
        let mut text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);
        Ok(Self {
            window,
            device,
            queue,
            surface,
            config,
            atlas,
            text_renderer,
        })
    }

    pub fn new(window: Rc<Window>) -> Result<Self, String> {
        pollster::block_on(Self::new_async(window))
    }

    pub fn render(
        &mut self,
        buffer: &mut Buffer,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) {
        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                font_system,
                &mut self.atlas,
                Resolution {
                    width: self.config.width,
                    height: self.config.height,
                },
                [TextArea {
                    buffer: &buffer,
                    left: 0.0,
                    top: 0.0,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: self.config.width as i32,
                        bottom: self.config.height as i32,
                    },
                    default_color: Color::rgb(255, 255, 255),
                }],
                swash_cache,
            )
            .unwrap();

        let frame = self.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.text_renderer.render(&self.atlas, &mut pass).unwrap();
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        self.atlas.trim();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }
}
