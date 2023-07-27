use wgpu_text::glyph_brush::OwnedSection;

use super::context::Renderable;

impl Renderable for OwnedSection {
    fn render<'a>(&'a self, ctx: &mut super::RenderContext<'a>) {
        ctx.text_brush
            .as_mut()
            .unwrap()
            .queue(ctx.device, ctx.queue, vec![self])
            .unwrap();
    }
}
