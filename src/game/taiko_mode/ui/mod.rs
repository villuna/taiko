use crate::game::taiko_mode::scene::NoteJudgement;
use crate::game::TextureCache;
use crate::render::text::BuildTextWithRenderer;
use crate::render::texture::{AnimatedSprite, AnimatedSpriteBuilder, Frame, Sprite, SpriteBuilder};
use crate::render::{rgb, Renderable, Renderer};
use kaku::{FontSize, HorizontalAlignment, Text, TextBuilder, VerticalAlignment};
use std::time::Instant;
use wgpu::RenderPass;

pub mod health_bar;
pub use health_bar::HealthBar;

// Positions.
// TODO: Replace this system something more sophisticated that respects resolution
pub const HEADER_HEIGHT: f32 = 315.;
pub const NOTE_FIELD_Y: f32 = HEADER_HEIGHT;
// The point on the screen where notes should be hit
pub const NOTE_HIT_X: f32 = 690.;
// The Y value where notes should be drawn
pub const NOTE_Y: f32 = NOTE_FIELD_Y + NOTE_FIELD_HEIGHT / 2.0;
pub const NOTE_FIELD_HEIGHT: f32 = 248.;
pub const LEFT_PANEL_WIDTH: f32 = 480.;

pub const TITLE_FONT_SIZE: f32 = 60.;

const JUDGEMENT_TEXT_DISPLAY_TIME: f32 = 0.5;
const JUDGEMENT_TEXT_Y: f32 = NOTE_Y - 50.;
const JUDGEMENT_TEXT_FLOAT_DIST: f32 = -20.;
const JUDGEMENT_TEXT_GOOD_COLOUR: [f32; 4] = [1., 202. / 255., 14. / 255., 1.];
const JUDGEMENT_TEXT_GOOD_OUTLINE_COLOUR: [f32; 4] = [37. / 255., 29. / 255., 0., 1.];
const JUDGEMENT_TEXT_OK_COLOUR: [f32; 4] = [1.; 4];
const JUDGEMENT_TEXT_OK_OUTLINE_COLOUR: [f32; 4] = [21. / 255., 21. / 255., 21. / 255., 1.];
const JUDGEMENT_TEXT_BAD_COLOUR: [f32; 4] = [46. / 255., 103. / 255., 209. / 255., 1.];
const JUDGEMENT_TEXT_BAD_OUTLINE_COLOUR: [f32; 4] = [0., 0., 0., 1.];

/// A UI element that displays some text indicating how well the player hit the last note.
/// The text is displayed for a short time while moving upwards, and becomes transparent as it ages.
pub struct JudgementText {
    judgement_sprites: [Text; 3],
    /// Contains the index of the current sprite, and the moment it was instantiated, or None if
    /// there's no currently visible sprite.
    current_sprite: Option<(usize, Instant)>,
}

impl JudgementText {
    pub fn new(renderer: &mut Renderer) -> Self {
        let mut build_judgement_text = |text, colour, outline_colour| {
            TextBuilder::new(
                text,
                renderer.font("mochiy pop one"),
                [NOTE_HIT_X, JUDGEMENT_TEXT_Y],
            )
            .font_size(Some(FontSize::Px(30.)))
            .horizontal_align(HorizontalAlignment::Center)
            .color(colour)
            .outlined(outline_colour, 3.)
            .build_text(renderer)
        };

        let judgement_sprites = [
            build_judgement_text(
                "Good",
                JUDGEMENT_TEXT_GOOD_COLOUR,
                JUDGEMENT_TEXT_GOOD_OUTLINE_COLOUR,
            ),
            build_judgement_text(
                "Ok",
                JUDGEMENT_TEXT_OK_COLOUR,
                JUDGEMENT_TEXT_OK_OUTLINE_COLOUR,
            ),
            build_judgement_text(
                "Bad",
                JUDGEMENT_TEXT_BAD_COLOUR,
                JUDGEMENT_TEXT_BAD_OUTLINE_COLOUR,
            ),
        ];

        Self {
            judgement_sprites,
            current_sprite: None,
        }
    }

    pub fn display_judgement(&mut self, judgement: NoteJudgement) {
        let index = judgement.index();
        self.current_sprite = Some((index, Instant::now()));
    }

    pub fn update(&mut self, renderer: &Renderer) {
        if let Some((index, instant)) = self.current_sprite {
            let elapsed = instant.elapsed().as_secs_f32();
            if elapsed > JUDGEMENT_TEXT_DISPLAY_TIME {
                // Time's up, so just disappear
                self.current_sprite = None;
                return;
            }

            let progress = elapsed / JUDGEMENT_TEXT_DISPLAY_TIME;
            let y = JUDGEMENT_TEXT_Y + JUDGEMENT_TEXT_FLOAT_DIST * 0.7 * (progress * 2. + 1.).ln();
            // This sets the position of the text relative to the starting position
            self.judgement_sprites[index].set_position([NOTE_HIT_X, y], &renderer.queue);
            // TODO: set transparency using a colour tint
        }
    }
}

impl Renderable for JudgementText {
    fn render<'pass>(&'pass self, renderer: &'pass Renderer, render_pass: &mut RenderPass<'pass>) {
        if let Some((index, _)) = self.current_sprite {
            self.judgement_sprites[index].render(renderer, render_pass);
        }
    }
}

/// Displays the hit count for the current drumroll
pub struct DrumrollDisplay {
    bg_bubble: Sprite,
    drumroll_message: Text,
    roll_number_text: Text,

    current_rolls: u64,
}

impl DrumrollDisplay {
    /// Creates a new drumroll count display, initialised to display nothing
    pub fn new(textures: &mut TextureCache, renderer: &mut Renderer) -> anyhow::Result<Self> {
        let bg_bubble = SpriteBuilder::new(textures.get(
            &renderer.device,
            &renderer.queue,
            "balloon speech bubble.png",
        )?)
        .position([575., 130.])
        .build(renderer);

        let drumroll_message =
            TextBuilder::new("Drumroll!", renderer.font("mplus bold"), [765., 190.])
                .color([1.; 4])
                .font_size(Some(FontSize::Px(40.)))
                .horizontal_align(HorizontalAlignment::Center)
                .vertical_align(VerticalAlignment::Top)
                .outlined([0., 0., 0., 1.], 3.)
                .build_text(renderer);

        let roll_number_text = TextBuilder::new("0", renderer.font("mochiy pop one"), [765., 240.])
            .color(rgb!(0xFF, 0x8E, 0x4B))
            .font_size(Some(FontSize::Px(80.)))
            .horizontal_align(HorizontalAlignment::Center)
            .vertical_align(VerticalAlignment::Top)
            .outlined(rgb!(0x60, 0x2B, 0x0C), 3.)
            .build_text(renderer);

        Ok(Self {
            bg_bubble,
            drumroll_message,
            roll_number_text,
            current_rolls: 0,
        })
    }

    /// Called every frame. Updates animations.
    pub fn update(&mut self, dt: f32) {
        // TODO
    }

    /// Increases the displayed drumroll count by one
    pub fn increment(&mut self, renderer: &mut Renderer) {
        self.current_rolls += 1;

        self.roll_number_text.set_text(
            format!("{}", self.current_rolls),
            &renderer.device,
            &renderer.queue,
            &mut renderer.text_renderer,
        );
    }

    /// Signals to the DrumrollDisplay that the current drumroll section is over. The display will
    /// stop displaying after a short delay and any calls to increment will reset the counter.
    pub fn finish(&mut self) {
        self.current_rolls = 0;
    }
}

impl Renderable for DrumrollDisplay {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        if self.current_rolls != 0 {
            self.bg_bubble.render(renderer, render_pass);
            self.drumroll_message.render(renderer, render_pass);
            self.roll_number_text.render(renderer, render_pass);
        }
    }
}

/// Displays the progress of a balloon roll as it is being played
/// visually, it appears to blow up a balloon, while showing how many hits are left
pub struct BalloonDisplay {
    bg_bubble: Sprite,
    drumroll_message: Text,
    roll_number_text: Text,
    balloon_sprite: AnimatedSprite,
    displaying: bool,
}

impl BalloonDisplay {
    pub fn new(textures: &mut TextureCache, renderer: &mut Renderer) -> anyhow::Result<Self> {
        // TODO: These are hard coded positions! Bad!
        let bg_bubble = SpriteBuilder::new(textures.get(
            &renderer.device,
            &renderer.queue,
            "balloon speech bubble.png",
        )?)
        .position([575., 130.])
        .build(renderer);

        let drumroll_message =
            TextBuilder::new("Drumroll!", renderer.font("mplus bold"), [765., 190.])
                .color([1.; 4])
                .font_size(Some(FontSize::Px(40.)))
                .horizontal_align(HorizontalAlignment::Center)
                .vertical_align(VerticalAlignment::Top)
                .outlined([0., 0., 0., 1.], 3.)
                .build_text(renderer);

        let roll_number_text = TextBuilder::new("0", renderer.font("mochiy pop one"), [765., 240.])
            .color(rgb!(0xFF, 0x8E, 0x4B))
            .font_size(Some(FontSize::Px(80.)))
            .horizontal_align(HorizontalAlignment::Center)
            .vertical_align(VerticalAlignment::Top)
            .outlined(rgb!(0x60, 0x2B, 0x0C), 3.)
            .build_text(renderer);

        let balloon_sprite = AnimatedSpriteBuilder::new(vec![
            Frame::new(
                textures.get(&renderer.device, &renderer.queue, "balloon 1.png")?,
                [50., 50.],
            ),
            Frame::new(
                textures.get(&renderer.device, &renderer.queue, "balloon 3.png")?,
                [50., 100.],
            ),
            Frame::new(
                textures.get(&renderer.device, &renderer.queue, "balloon 5.png")?,
                [50., 150.],
            ),
        ])
        .position([NOTE_HIT_X, NOTE_Y])
        .build(renderer);

        Ok(Self {
            bg_bubble,
            drumroll_message,
            balloon_sprite,
            roll_number_text,
            displaying: false,
        })
    }

    /// Plays the animation for when the drumroll is over but the balloon hasn't been popped
    pub fn discard(&mut self) {
        // TODO
        self.displaying = false;
    }

    /// Displays the balloon and number of hits left
    pub fn hit(&mut self, hits_left: u32, hit_target: u32, renderer: &mut Renderer) {
        if !self.displaying {
            self.displaying = true;
        }

        if hits_left == 0 {
            self.displaying = false;
        }

        self.roll_number_text.set_text(
            format!("{hits_left}"),
            &renderer.device,
            &renderer.queue,
            &mut renderer.text_renderer,
        );

        let ratio = hits_left as f32 / hit_target as f32;

        let image_index = if ratio > 0.8 {
            0
        } else if ratio > 0.4 {
            1
        } else {
            2
        };

        self.balloon_sprite.set_index(image_index, renderer);
    }

    /// Plays the animation for popping the balloon
    fn pop(&mut self) {
        // TODO
        self.displaying = false;
    }

    /// Updates the animated sprites
    pub fn update(&mut self, _delta_time: f32) {
        // TODO
    }
}

impl Renderable for BalloonDisplay {
    fn render<'pass>(&'pass self, renderer: &'pass Renderer, render_pass: &mut RenderPass<'pass>) {
        if self.displaying {
            self.balloon_sprite.render(renderer, render_pass);
            self.bg_bubble.render(renderer, render_pass);
            self.drumroll_message.render(renderer, render_pass);
            self.roll_number_text.render(renderer, render_pass);
        }
    }
}
