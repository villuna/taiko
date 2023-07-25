use wgpu_text::glyph_brush::OwnedSection;

use super::context::Renderable;

pub struct Text {
    pub section: OwnedSection,
}

impl Renderable for Text {
    fn render<'a>(&'a self, ctx: &mut super::RenderContext<'a>) {
        ctx.text_brush
            .as_mut()
            .unwrap()
            .queue(ctx.device, ctx.queue, vec![&self.section])
            .unwrap();
    }
}
