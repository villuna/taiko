use kaku::{Text, TextBuilder};

use crate::{
    game::{ui_elements::Button, Context, GameState, RenderContext, StateTransition, TextureCache},
    render::{
        shapes::{LinearGradient, Shape, ShapeBuilder, SolidColour},
        texture::{Sprite, SpriteBuilder},
        Renderer,
    },
};

use super::SongSelect;

pub struct MainMenu {
    background: Sprite,
    gradient: Shape,
    menu_frame: Shape,
    title: Text,
    taiko_mode_button: Button,
    settings_button: Button,
    exit_button: Button,
}

impl MainMenu {
    pub fn new(textures: &mut TextureCache, renderer: &mut Renderer) -> anyhow::Result<Self> {
        let gradient = ShapeBuilder::new()
            .filled_rectangle([0., 0.], [680., 1080.], SolidColour::new([0., 0., 0., 0.8]))?
            .filled_rectangle(
                [680., 0.],
                [940., 1080.],
                LinearGradient::new([0., 0., 0., 0.8], [0.; 4], [680., 0.], [940., 0.]).unwrap(),
            )?
            .build(&renderer.device);

        let menu_frame = ShapeBuilder::new()
            .filled_roundrect(
                [40., 40.],
                [640., 1040.],
                12.,
                SolidColour::new([40. / 255., 40. / 255., 40. / 255., 0.95]),
            )?
            .build(&renderer.device);

        let title = TextBuilder::new("Unnamed Taiko\nSimulator!", renderer.font("mplus bold"), [100., 95.])
            .color([141. / 255., 64. / 255., 1., 1.])
            .build(&renderer.device, &renderer.queue, &mut renderer.text_renderer);

        let taiko_mode_button = Button::new(
            "Taiko Mode",
            [100., 290.],
            [290., 65.],
            SolidColour::new([120. / 255., 29. / 255., 29. / 255., 1.]),
            30.,
            renderer,
        )?;

        let settings_button = Button::new(
            "Settings",
            [100., 370.],
            [290., 65.],
            SolidColour::new([43. / 255., 111. / 255., 27. / 255., 1.]),
            30.,
            renderer,
        )?;

        let exit_button = Button::new(
            "Exit",
            [100., 930.],
            [150., 50.],
            SolidColour::new([72. / 255., 72. / 255., 72. / 255., 1.]),
            20.,
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
            gradient,
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
        ctx.render(&self.gradient);
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
