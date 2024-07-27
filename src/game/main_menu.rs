use kaku::{FontSize, HorizontalAlignment, Text, TextBuilder, VerticalAlignment};

use crate::{
    game::{ui_elements::{Button, ButtonOptions}, Context, GameState, RenderContext, StateTransition, TextureCache},
    render::{
        rgb,
        shapes::{LinearGradient, Shape, ShapeBuilder, SolidColour},
        texture::{Sprite, SpriteBuilder},
        Renderer,
    },
};

use super::SongSelect;

pub struct MainMenu {
    background: Sprite,
    menu_frame: Shape,
    title: Text,
    taiko_mode_button: Button,
    settings_button: Button,
    exit_button: Button,
}

impl MainMenu {
    pub fn new(textures: &mut TextureCache, renderer: &mut Renderer) -> anyhow::Result<Self> {
        let menu_frame = ShapeBuilder::new()
            .filled_roundrect(
                [40., 40.],
                // It really goes to (640, 280) but the bottom bit will be covered by another
                // rectangle
                [640., 330.],
                50.,
                LinearGradient::new(
                    rgb!(0x1E, 0x43, 0xC6),
                    rgb!(0x96, 0x5A, 0xE1),
                    [40., 40.],
                    [40., 280.],
                )
                .unwrap(),
            )?
            .filled_roundrect(
                [40., 940.],
                [640., 1040.],
                50.,
                SolidColour::new(rgb!(0xFF, 0xEB, 0xCE))
            )?
            .filled_rectangle(
                [40., 280.],
                [640., 990.],
                SolidColour::new(rgb!(0xFF, 0xEB, 0xCE)),
            )?
            .stroke_roundrect(
                [40., 40.],
                [640., 1040.],
                50.,
                SolidColour::new(rgb!(0, 0, 0.)),
                5.
            )?
            .build(&renderer.device);

        let title = TextBuilder::new(
            "Unnamed Taiko\nSimulator Demo!",
            renderer.font("mochiy pop one"),
            [340., 90.],
        )
        .font_size(Some(FontSize::Px(50.)))
        .vertical_align(VerticalAlignment::Top)
        .horizontal_align(HorizontalAlignment::Center)
        .color([1.; 4])
        .outlined(rgb!(0x14, 0x10, 0x6D), 3.)
        .build(
            &renderer.device,
            &renderer.queue,
            &mut renderer.text_renderer,
        );

        let taiko_mode_button = Button::new(
            "Taiko Mode",
            [120., 320.],
            ButtonOptions {
                colour: rgb!(0xEC, 0x45, 0x20),
                text_outline_colour: rgb!(0x72, 0x19, 0x19),
                ..Default::default()
            },
            renderer,
        )?;

        let settings_button = Button::new(
            "Settings",
            [120., 440.],
            ButtonOptions {
                colour: rgb!(0x04, 0xDF, 0x00),
                text_outline_colour: rgb!(0x0A, 0x54, 0x16),
                ..Default::default()
            },
            renderer,
        )?;

        let exit_button = Button::new(
            "Exit",
            [120., 940.],
            ButtonOptions {
                colour: rgb!(0x50, 0x17, 0xBB),
                size: [170., 50.],
                font_size: FontSize::Px(25.),
                ..Default::default()
            },
            renderer,
        )?;

        let background = SpriteBuilder::new(textures.get(
            &renderer.device,
            &renderer.queue,
            "song_select_bg.jpg",
        )?)
        .build(renderer);

        Ok(MainMenu {
            background,
            menu_frame,
            title,
            taiko_mode_button,
            settings_button,
            exit_button,
        })
    }
}

impl GameState for MainMenu {
    fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        ctx.render(&self.background);
        ctx.render(&self.menu_frame);
        ctx.render(&self.title);
        ctx.render(&self.taiko_mode_button);
        ctx.render(&self.settings_button);
        ctx.render(&self.exit_button);
    }

    fn update(&mut self, ctx: &mut Context, _delta_time: f32) -> StateTransition {
        self.taiko_mode_button.update(ctx);
        self.settings_button.update(ctx);
        self.exit_button.update(ctx);

        if self.taiko_mode_button.is_clicked(ctx) {
            StateTransition::Push(Box::new(
                SongSelect::new(ctx.textures, ctx.renderer).unwrap(),
            ))
        } else if self.exit_button.is_clicked(ctx) {
            StateTransition::Exit
        } else {
            StateTransition::Continue
        }
    }
}
