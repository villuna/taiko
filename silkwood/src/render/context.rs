/// A handle to a [wgpu::RenderPass] and all the related resources needed to render things in the
/// render pass.
pub struct RenderPassContext<'pass> {
    pub render_pass: wgpu::RenderPass<'pass>,
    pub device: &'pass wgpu::Device,
    pub queue: &'pass wgpu::Queue,
    pub pipeline_cache: &'pass Vec<(&'static str, wgpu::RenderPipeline)>,
}

/// A trait that allows objects to render themselves to the screen in any given render pass. If a
/// type implements Renderable, then it is able to be rendered by the [RenderContext]'s render
/// function.
pub trait Renderable {
    fn render<'pass>(&'pass self, ctx: &mut RenderPassContext<'pass>);
}

impl<'pass> RenderPassContext<'pass> {
    /// Renders the target object in the current render pass using its [Renderable] implementation.
    pub fn render<R: Renderable>(&mut self, target: &'pass R) {
        target.render(self);
    }

    /// Gets the render pipeline referred to by the given name, if it exists.
    pub fn pipeline(&self, name: &str) -> Option<&'pass wgpu::RenderPipeline> {
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
