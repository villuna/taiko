use crate::app::taiko_mode::scene::NoteJudgement;
use crate::app::{RenderContext, TextureCache};
use crate::render::shapes::{LinearGradient, Shape, ShapeBuilder, SolidColour};
use crate::render::text::Text;
use crate::render::texture::{AnimatedSprite, AnimatedSpriteBuilder, Frame, Sprite, SpriteBuilder};
use crate::render::{Renderable, Renderer};
use lyon::geom::point;
use lyon::lyon_tessellation::{BuffersBuilder, StrokeOptions};
use lyon::path::Path;
use std::time::Instant;
use wgpu::RenderPass;
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, SectionBuilder, VerticalAlign};

use super::note::{TaikoModeBarline, TaikoModeNote};

// Colours
pub const HEADER_TOP_COL: [f32; 4] = [30. / 255., 67. / 255., 198. / 255., 0.94];
pub const HEADER_BOTTOM_COL: [f32; 4] = [150. / 255., 90. / 255., 225. / 255., 1.];
pub const NOTE_FIELD_COL: [f32; 4] = [45. / 255., 45. / 255., 45. / 255., 1.];
pub const CREAM: [f32; 4] = [1., 235. / 255., 206. / 255., 1.];
pub const RECEPTACLE_COL: [f32; 4] = [0.26, 0.26, 0.26, 1.0];
pub const LEFT_PANEL_TOP_COL: [f32; 4] = [1., 73. / 255., 73. / 255., 1.];
pub const LEFT_PANEL_BOTTOM_COL: [f32; 4] = [229. / 255., 41. / 255., 41. / 255., 1.];

// Positions.
// TODO: Replace this system something more sophisticated that respects resolution
pub const HEADER_HEIGHT: f32 = 315.;
pub const SPACER_WIDTH: f32 = 8.;
pub const NOTE_FIELD_Y: f32 = HEADER_HEIGHT + SPACER_WIDTH;
// The point on the screen where notes should be hit
pub const NOTE_HIT_X: f32 = 690.;
// The Y value where notes should be drawn
pub const NOTE_Y: f32 = NOTE_FIELD_Y + NOTE_FIELD_HEIGHT / 2.0;
pub const NOTE_FIELD_HEIGHT: f32 = 232.;
pub const LEFT_PANEL_WIDTH: f32 = 480.;

pub struct Header {
    background: Shape,
    title: Text,
}

impl Header {
    pub fn new(renderer: &mut Renderer, title: &str) -> anyhow::Result<Self> {
        let background = ShapeBuilder::new()
            .filled_rectangle(
                [0., 0.],
                [1920., HEADER_HEIGHT],
                LinearGradient::new(
                    HEADER_TOP_COL,
                    HEADER_BOTTOM_COL,
                    [0., 0.],
                    [0., HEADER_HEIGHT],
                )
                .ok_or(anyhow::format_err!("cant construct linear gradient"))?,
            )?
            .build(&renderer.device);

        let title = SectionBuilder::default()
            .with_screen_position((1840.0, 20.0))
            .with_layout(
                Layout::default()
                    .h_align(HorizontalAlign::Right)
                    .v_align(VerticalAlign::Top),
            )
            .with_text(vec![wgpu_text::glyph_brush::Text::new(title)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(120.0)]);

        let title = Text::new_outlined(renderer, &title)?;

        Ok(Self { background, title })
    }

    pub fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        ctx.render(&self.background);
        ctx.render(&self.title);
    }
}

pub struct NoteField {
    field: Shape,
    left_panel: Shape,
}

impl NoteField {
    pub fn new(renderer: &mut Renderer) -> anyhow::Result<Self> {
        let field = ShapeBuilder::new()
            // Background
            .filled_rectangle(
                [0., NOTE_FIELD_Y],
                [1920., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                SolidColour::new(NOTE_FIELD_COL),
            )?
            // Top spacer
            .filled_rectangle(
                [0., HEADER_HEIGHT],
                [1920., HEADER_HEIGHT + SPACER_WIDTH],
                SolidColour::new(CREAM),
            )?
            // Bottom spacer
            .filled_rectangle(
                [0., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                [1920., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT + SPACER_WIDTH],
                SolidColour::new(CREAM),
            )?
            // Note recepticle
            .stroke_shape(|tess, out| {
                let mut path = Path::builder();
                path.begin(point(NOTE_HIT_X, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0));
                path.line_to(point(NOTE_HIT_X, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0));
                path.end(false);

                let options = StrokeOptions::DEFAULT.with_line_width(4.0);
                let mut builder = BuffersBuilder::new(out, SolidColour::new(RECEPTACLE_COL));

                // A line that shows exactly where notes should be hit
                tess.tessellate_path(&path.build(), &options, &mut builder)?;

                // The outline of a small note
                tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 50.0, &options, &mut builder)?;

                // The outline of a large note
                tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 75.0, &options, &mut builder)?;

                Ok(())
            })?
            .build(&renderer.device);

        let left_panel = ShapeBuilder::new()
            .filled_rectangle(
                [0.0, NOTE_FIELD_Y],
                [LEFT_PANEL_WIDTH, NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                LinearGradient::new(
                    LEFT_PANEL_TOP_COL,
                    LEFT_PANEL_BOTTOM_COL,
                    [0.0, NOTE_FIELD_Y],
                    [0.0, NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                )
                .ok_or(anyhow::format_err!("couldnt construct linear gradient"))?,
            )?
            .filled_rectangle(
                [LEFT_PANEL_WIDTH, NOTE_FIELD_Y],
                [LEFT_PANEL_WIDTH + 3., NOTE_FIELD_Y + NOTE_FIELD_HEIGHT],
                SolidColour::new([0., 0., 0., 1.]),
            )?
            .build(&renderer.device);

        Ok(Self { field, left_panel })
    }

    pub fn render<'pass>(
        &'pass mut self,
        ctx: &mut RenderContext<'_, 'pass>,
        notes: impl Iterator<Item = &'pass TaikoModeNote>,
        barlines: impl Iterator<Item = &'pass TaikoModeBarline>,
    ) {
        ctx.render(&self.field);

        // Thankfully barlines are all drawn before all the notes
        // so we don't have to worry about ordering shenanigans :D
        for b in barlines {
            ctx.render(b);
        }

        for n in notes {
            ctx.render(n);
        }

        ctx.render(&self.left_panel);
    }
}

const JUDGEMENT_TEXT_DISPLAY_TIME: f32 = 0.5;
const JUDGEMENT_TEXT_Y: f32 = NOTE_Y - 50.0;
const JUDGEMENT_TEXT_FLOAT_DIST: f32 = -20.0;
const JUDGEMENT_TEXT_GOOD_COLOUR: [f32; 4] = [1.0, 198.0 / 255.0, 41.0 / 255.0, 1.0];
const JUDGEMENT_TEXT_OK_COLOUR: [f32; 4] = [1.0; 4];
const JUDGEMENT_TEXT_BAD_COLOUR: [f32; 4] = [46.0 / 255.0, 103.0 / 255.0, 209.0 / 255.0, 1.0];

// TODO: Japanese localisation
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
        let mut build_judgement_text = |text, colour| {
            let section = SectionBuilder::default()
                .with_screen_position((NOTE_HIT_X, JUDGEMENT_TEXT_Y))
                .with_layout(
                    Layout::default()
                        .h_align(HorizontalAlign::Center)
                        .v_align(VerticalAlign::Bottom),
                )
                .with_text(vec![wgpu_text::glyph_brush::Text::new(text)
                    .with_color(colour)
                    .with_scale(50.0)]);

            Text::new_outlined(renderer, &section).unwrap()
        };

        let judgement_sprites = [
            build_judgement_text("Good", JUDGEMENT_TEXT_GOOD_COLOUR),
            build_judgement_text("Ok", JUDGEMENT_TEXT_OK_COLOUR),
            build_judgement_text("Bad", JUDGEMENT_TEXT_BAD_COLOUR),
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
            let y = JUDGEMENT_TEXT_FLOAT_DIST * progress;
            // This sets the position of the text relative to the starting position
            // TODO: Refactor the text system to not be so jank
            self.judgement_sprites[index]
                .sprite
                .set_position([0.0, y], renderer);
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

/// Displays the progress of a balloon roll as it is being played
/// visually, it appears to blow up a balloon, while showing how many hits are left
pub struct BalloonDisplay {
    bg_bubble: Sprite,
    drumroll_message: Text,
    balloon_sprite: AnimatedSprite,
    displaying: bool,
    // TODO: Text indicating the number of rolls left
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

        let section = SectionBuilder::default()
            .with_screen_position((765., 190.))
            .with_layout(
                Layout::default()
                    .h_align(HorizontalAlign::Center)
                    .v_align(VerticalAlign::Bottom),
            )
            .with_text(vec![wgpu_text::glyph_brush::Text::new("Drumroll!")
                .with_color([1.0; 4])
                .with_scale(32.0)]);

        let drumroll_message = Text::new_outlined(renderer, &section)?;

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
            displaying: false,
        })
    }

    /// Plays the animation for when the drumroll is over but the balloon hasn't been popped
    pub fn discard(&mut self) {
        // TODO
        self.displaying = false;
    }

    /// Displays the balloon and number of hits left
    pub fn hit(&mut self, hits_left: u32, _hit_target: u32) {
        if !self.displaying {
            self.displaying = true;
        }

        if hits_left == 0 {
            self.displaying = false;
        }
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
        }
    }
}
