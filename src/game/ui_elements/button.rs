use crate::game::Context;
use crate::render::shapes::{Shape, ShapeBuilder, ShapeVertex, SolidColour};
use crate::render::text::BuildTextWithRenderer;
use crate::render::Renderable;
use crate::render::Renderer;
use kaku::{FontSize, Text, TextBuilder};
use lyon::tessellation::FillVertexConstructor;
use winit::event::MouseButton;

pub struct Button {
    pos: [f32; 2],
    size: [f32; 2],
    mouse_entered: bool,
    bg: Shape,
    outline: Shape,
    hover_overlay: Shape,
    text: Text,
    shadow: Shape,
}

impl Button {
    pub fn new<C>(
        text: &str,
        pos: [f32; 2],
        size: [f32; 2],
        col: C,
        font_size: FontSize,
        renderer: &mut Renderer,
    ) -> anyhow::Result<Self>
    where
        C: FillVertexConstructor<ShapeVertex> + Clone,
    {
        let text_position = [pos[0] + size[0] / 2., pos[1] + size[1] / 2.];
        let text = TextBuilder::new(text, renderer.font("mplus regular"), text_position)
            .color([1.; 4])
            .font_size(Some(font_size))
            .horizontal_align(kaku::HorizontalAlignment::Center)
            .vertical_align(kaku::VerticalAlignment::Middle)
            .build_text(renderer);

        let bg = ShapeBuilder::new()
            .position([pos[0], pos[1], 0.])
            .filled_roundrect([0., 0.], size, 12., col)?
            .build(&renderer.device);

        let hover_overlay = ShapeBuilder::new()
            .position([pos[0], pos[1], 0.])
            .filled_roundrect([0., 0.], size, 12., SolidColour::new([1., 1., 1., 0.02]))?
            .build(&renderer.device);

        let outline = ShapeBuilder::new()
            .position([pos[0], pos[1], 0.])
            .stroke_roundrect([0., 0.], size, 12., SolidColour::new([220. / 255.; 4]), 3.)?
            .build(&renderer.device);

        let shadow = ShapeBuilder::new()
            .position([pos[0] + 1., pos[1] + 1., 0.])
            .filled_roundrect(
                [0., 0.],
                [size[0] + 3., size[1] + 3.],
                12.,
                SolidColour::new([0.0, 0.0, 0.0, 0.2]),
            )?
            .build(&renderer.device);

        Ok(Button {
            pos,
            size,
            mouse_entered: false,
            bg,
            outline,
            hover_overlay,
            text,
            shadow,
        })
    }

    pub fn update(&mut self, ctx: &mut Context) {
        self.mouse_entered = ctx.mouse.cursor_pos().is_some_and(|(x, y)| {
            x >= self.pos[0]
                && x <= self.pos[0] + self.size[0]
                && y >= self.pos[1]
                && y <= self.pos[1] + self.size[1]
        });
    }

    pub fn is_clicked(&mut self, ctx: &mut Context) -> bool {
        self.mouse_entered && ctx.mouse.is_just_pressed(MouseButton::Left)
    }
}

impl Renderable for Button {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.shadow.render(renderer, render_pass);
        self.bg.render(renderer, render_pass);
        self.text.render(renderer, render_pass);

        if self.mouse_entered {
            self.hover_overlay.render(renderer, render_pass);
            self.outline.render(renderer, render_pass);
        }
    }
}
