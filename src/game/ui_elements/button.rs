use crate::game::Context;
use crate::render::shapes::{Shape, ShapeBuilder, SolidColour};
use crate::render::text::BuildTextWithRenderer;
use crate::render::{rgb, Renderable};
use crate::render::Renderer;
use kaku::{FontSize, Text, TextBuilder};
use winit::event::MouseButton;

pub struct Button {
    pos: [f32; 2],
    size: [f32; 2],
    mouse_entered: bool,
    bg: Shape,
    outline: Shape,
    hover_outline: Shape,
    hover_overlay: Shape,
    text: Text,
    shadow: Shape,
}

#[derive(Clone, Copy, Debug)]
pub struct ButtonOptions {
    pub size: [f32; 2],
    pub colour: [f32; 4],
    pub text_colour: [f32; 4],
    pub text_outline_colour: [f32; 4],
    pub text_outline_width: f32,
    pub font_size: FontSize,
}

impl Default for ButtonOptions {
    fn default() -> Self {
        Self {
            size: [420., 80.],
            colour: rgb!(0xFF, 0xFF, 0xFF),
            text_colour: rgb!(0xFF, 0xFF, 0xFF),
            text_outline_colour: rgb!(0, 0, 0),
            text_outline_width: 2.,
            font_size: FontSize::Px(40.),
        }
    }
}

impl Button {
    pub fn new(
        text: &str,
        pos: [f32; 2],
        options: ButtonOptions,
        renderer: &mut Renderer,
    ) -> anyhow::Result<Self> {
        let text_position = [pos[0] + options.size[0] / 2., pos[1] + options.size[1] / 2.];
        let text = TextBuilder::new(text, renderer.font("mplus bold"), text_position)
            .color(options.text_colour)
            .font_size(Some(options.font_size))
            .horizontal_align(kaku::HorizontalAlignment::Center)
            .vertical_align(kaku::VerticalAlignment::Middle)
            .outlined(options.text_outline_colour, options.text_outline_width)
            .build_text(renderer);

        let bg = ShapeBuilder::new()
            .position([pos[0], pos[1], 0.])
            .filled_roundrect([0., 0.], options.size, 12., SolidColour::new(options.colour))?
            .build(&renderer.device);

        let hover_overlay = ShapeBuilder::new()
            .position([pos[0], pos[1], 0.])
            .filled_roundrect([0., 0.], options.size, 12., SolidColour::new([1., 1., 1., 0.1]))?
            .build(&renderer.device);

        let outline = ShapeBuilder::new()
            .position([pos[0], pos[1], 0.])
            .stroke_roundrect([0., 0.], options.size, 12., SolidColour::new(rgb!(0x24, 0x24, 0x24)), 3.)?
            .build(&renderer.device);

        let hover_outline = ShapeBuilder::new()
            .position([pos[0] + 1., pos[1] + 1., 0.])
            .stroke_roundrect([0., 0.], [options.size[0] - 2., options.size[1] - 2.], 12., SolidColour::new([1.; 4]), 2.)?
            .build(&renderer.device);

        let shadow = ShapeBuilder::new()
            .position([pos[0] + 1., pos[1] + 1., 0.])
            .filled_roundrect(
                [0., 0.],
                [options.size[0] + 3., options.size[1] + 3.],
                12.,
                SolidColour::new([0.0, 0.0, 0.0, 0.2]),
            )?
            .build(&renderer.device);

        Ok(Button {
            pos,
            size: options.size,
            mouse_entered: false,
            bg,
            outline,
            hover_outline,
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
        self.outline.render(renderer, render_pass);

        if self.mouse_entered {
            self.hover_overlay.render(renderer, render_pass);
            self.hover_outline.render(renderer, render_pass);
        }

        self.text.render(renderer, render_pass);
    }
}
