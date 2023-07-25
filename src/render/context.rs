pub struct RenderContext<'a> {
    pub render_pass: wgpu::RenderPass<'a>,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub pipeline_cache: &'a Vec<(&'static str, wgpu::RenderPipeline)>,
    pub text_brush: Option<&'a mut wgpu_text::TextBrush>,
}

pub trait Renderable {
    fn render<'a>(&'a self, ctx: &mut RenderContext<'a>);
}

impl<'a> RenderContext<'a> {
    pub fn render<R: Renderable>(&mut self, target: &'a R) {
        target.render(self);
    }

    pub fn pipeline(&self, name: &str) -> Option<&'a wgpu::RenderPipeline> {
        self.pipeline_cache.iter()
            .find_map(|(n, pipeline)| {
                if name == *n {
                    Some(pipeline)
                } else {
                    None
                }
            })
    }
}
