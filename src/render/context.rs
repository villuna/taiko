/// A handle to a [wgpu::RenderPass] and all the related resources needed to render things in the
/// render pass.
#[non_exhaustive]
pub struct RenderContext<'a> {
    pub render_pass: wgpu::RenderPass<'a>,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub pipeline_cache: &'a Vec<(&'static str, wgpu::RenderPipeline)>,
    pub text_brush: Option<&'a mut wgpu_text::TextBrush>,
}

/// A trait that allows objects to render themselves to the screen in any given render pass. If a
/// type implements Renderable, then it is able to be rendered by the [RenderContext]'s render
/// function.
pub trait Renderable {
    fn render<'a>(&'a self, ctx: &mut RenderContext<'a>);
}

impl<'a> RenderContext<'a> {
    /// Renders the target object in the current render pass using its [Renderable] implementation.
    pub fn render<R: Renderable>(&mut self, target: &'a R) {
        target.render(self);
    }

    /// Gets the render pipeline referred to by the given name, if it exists.
    pub fn pipeline(&self, name: &str) -> Option<&'a wgpu::RenderPipeline> {
        self.pipeline_cache.iter().find_map(
            |(n, pipeline)| {
                if name == *n {
                    Some(pipeline)
                } else {
                    None
                }
            },
        )
    }
}
