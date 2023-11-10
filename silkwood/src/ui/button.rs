use wgpu_text::glyph_brush::{SectionBuilder, Layout, HorizontalAlign, VerticalAlign};

use crate::render::{primitives::{Primitive, SolidColour}, Renderer, context::Renderable};

use super::text::Text;

pub struct Button {
    main_rectangle: Primitive,
    main_outline: Primitive,
    main_text: Text,
}

impl Button {
    pub fn new(
        renderer: &mut Renderer,
        text: &str,
        position: [f32; 2],
        dimensions: [f32; 2],
        colour: [f32; 4],
    ) -> Self {
        let moved_position = [position[0] - dimensions[0] / 2.0, position[1] - dimensions[1] / 2.0, 0.0];
        let main_rectangle = Primitive::filled_roundrect(
            &renderer.device,
            moved_position,
            dimensions,
            20.0,
            false,
            SolidColour::new(colour),
        )
        .unwrap();

        let main_outline = Primitive::stroke_roundrect(
            &renderer.device,
            moved_position,
            dimensions,
            20.0,
            false,
            SolidColour::new([1.0; 4]),
            7.0,
        )
        .unwrap();

        let main_text = SectionBuilder::default()
            .with_screen_position((position[0], position[1]))
            .with_layout(Layout::default().h_align(HorizontalAlign::Center).v_align(VerticalAlign::Center))
            .with_text(vec![wgpu_text::glyph_brush::Text::new(text)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(80.0)]);

        let main_text = Text::new_outlined(renderer, &main_text).unwrap();

        Self { main_rectangle, main_outline, main_text }
    }
}

impl Renderable for Button {
    fn render<'pass>(&'pass self, ctx: &mut crate::render::RenderPassContext<'pass>) {
        self.main_rectangle.render(ctx);
        self.main_outline.render(ctx);
        self.main_text.render(ctx);
    }
}
