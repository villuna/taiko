use kaku::{Text, TextBuilder};

use super::{Renderable, Renderer};

impl Renderable for Text {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass super::Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        renderer.text_renderer.draw_text(render_pass, &self);
    }
}

pub(crate) trait BuildTextWithRenderer {
    fn build_text(&self, renderer: &mut Renderer) -> Text;
}

impl BuildTextWithRenderer for TextBuilder {
    fn build_text(&self, renderer: &mut Renderer) -> Text {
        self.build(&renderer.device, &renderer.queue, &mut renderer.text_renderer)
    }
}
