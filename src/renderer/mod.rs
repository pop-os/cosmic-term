use cosmic_text::{Buffer, FontSystem, SwashCache};
use std::rc::Rc;
use winit::window::Window;

pub use self::software::SoftwareRenderer;
pub mod software;

#[cfg(feature = "wgpu")]
pub use self::wgpu::WgpuRenderer;
#[cfg(feature = "wgpu")]
pub mod wgpu;

pub enum Renderer {
    Software(SoftwareRenderer),
    #[cfg(feature = "wgpu")]
    Wgpu(WgpuRenderer),
}

impl Renderer {
    pub fn new(window: Rc<Window>) -> Result<Self, String> {
        #[cfg(feature = "wgpu")]
        match WgpuRenderer::new(window.clone()) {
            Ok(renderer) => return Ok(Self::Wgpu(renderer)),
            Err(err) => {
                log::error!("failed to use hardware rendering: {}", err);
            }
        }

        SoftwareRenderer::new(window).map(Renderer::Software)
    }

    pub fn render(
        &mut self,
        buffer: &mut Buffer,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) {
        match self {
            Self::Software(renderer) => {
                renderer.render(buffer, font_system, swash_cache);
            }
            #[cfg(feature = "wgpu")]
            Self::Wgpu(renderer) => {
                renderer.render(buffer, font_system, swash_cache);
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        match self {
            Self::Software(renderer) => {
                renderer.resize(width, height);
            }
            #[cfg(feature = "wgpu")]
            Self::Wgpu(renderer) => {
                renderer.resize(width, height);
            }
        }
    }
}
