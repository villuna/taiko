use anyhow::Context;
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, SectionBuilder, VerticalAlign};

use crate::render::{
    context::Renderable,
    shapes::{Shape, ShapeBuilder, SolidColour},
    Renderer,
};

use super::text::Text;

pub struct Button {
    main_rect: Shape,
    main_text: Text,
}

impl Button {
    pub fn new(
        renderer: &mut Renderer,
        text: &str,
        position: [f32; 2],
        dimensions: [f32; 2],
        colour: [f32; 4],
    ) -> anyhow::Result<Self> {
        let moved_position = [
            position[0] - dimensions[0] / 2.0,
            position[1] - dimensions[1] / 2.0,
            0.0,
        ];
        let main_rect = ShapeBuilder::new()
            .position(moved_position)
            .filled_roundrect([0., 0.], dimensions, 20.0, SolidColour::new(colour))
            .context("creating fill for button")?
            .stroke_roundrect([0., 0.], dimensions, 20.0, SolidColour::new([1.0; 4]), 7.0)
            .context("creating outline for button")?
            .build(&renderer.device);

        let main_text = SectionBuilder::default()
            .with_screen_position((position[0], position[1]))
            .with_layout(
                Layout::default()
                    .h_align(HorizontalAlign::Center)
                    .v_align(VerticalAlign::Center),
            )
            .with_text(vec![wgpu_text::glyph_brush::Text::new(text)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(80.0)]);

        let main_text =
            Text::new_outlined(renderer, &main_text).context("creating text for button")?;

        Ok(Self {
            main_rect,
            main_text,
        })
    }
}

impl Renderable for Button {
    fn render<'pass>(&'pass self, ctx: &mut crate::render::RenderPassContext<'pass>) {
        self.main_rect.render(ctx);
        self.main_text.render(ctx);
    }
}
