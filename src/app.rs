use winit::window::Window;

use crate::renderer::Renderer;

pub struct App {
    pub renderer: Renderer,
}

impl App {
    pub async fn new(window: Window) -> anyhow::Result<Self> {
        Ok(Self {
            renderer: Renderer::new(window).await?,
        })
    }

    pub fn update(&mut self, _delta: f32) {}

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.renderer.render(Some(wgpu::Color::BLACK))
    }
}
