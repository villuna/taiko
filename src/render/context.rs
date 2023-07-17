use super::Renderer;

pub struct RenderContext<'a> {
    pub render_pass: wgpu::RenderPass<'a>,
    // This needs to exist so that the buffers live as long as the render pass.
    temp_buffers: Vec<wgpu::Buffer>,
    pub renderer: &'a Renderer,
}

pub trait Renderable {
    fn render<'a>(&'a self, ctx: &mut RenderContext<'a>);
}

impl<'a> RenderContext<'a> {
    pub fn new(render_pass: wgpu::RenderPass<'a>, renderer: &'a Renderer) -> Self {
        Self {
            render_pass,
            temp_buffers: Vec::new(),
            renderer,
        }
    }

    pub fn render<R: Renderable>(&mut self, target: &'a R) {
        target.render(self);
    }
}
