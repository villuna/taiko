//! Defines structs for drawing notes and barlines to the screen
use lyon::lyon_tessellation::TessellationError;

use crate::beatmap_parser::track::NoteType;
use crate::beatmap_parser::{Barline, Note};
use crate::render::texture::SpriteBuilder;
use crate::render::Renderer;
use crate::{app::TextureCache, render::shapes::ShapeBuilder};

use crate::render::{
    shapes::{Shape, SolidColour},
    texture::Sprite,
    Renderable,
};

use super::ui::{LEFT_PANEL_WIDTH, NOTE_FIELD_HEIGHT, NOTE_FIELD_Y, NOTE_HIT_X, NOTE_Y};

const VELOCITY: f32 = (1920. - NOTE_HIT_X) / 2.;
const ROLL_COLOUR: [f32; 4] = [1., 195. / 255., 44. / 255., 1.];

/// Takes a list of notes in a song and creates visual representations for all of them.
pub fn create_notes(
    renderer: &Renderer,
    textures: &mut TextureCache,
    notes: &[Note],
) -> Vec<TaikoModeNote> {
    notes
        .iter()
        .filter_map(|note| TaikoModeNote::new(renderer, note, textures))
        .collect()
}

/// Takes a list of barlines in a song and creates visual representations for all of them.
pub fn create_barlines(renderer: &mut Renderer, barlines: &[Barline]) -> Vec<TaikoModeBarline> {
    barlines
        .iter()
        .map(|barline| {
            let visual_line = ShapeBuilder::new()
                .filled_rectangle(
                    [-1., 0.],
                    [1., NOTE_FIELD_HEIGHT],
                    SolidColour::new([1., 1., 1., 0.5]),
                )
                .expect("Error creating barline shape")
                .position([
                    x_position_of_note(barline.time, 0., barline.scroll_speed),
                    NOTE_FIELD_Y,
                    0.,
                ])
                .build(&renderer.device);

            TaikoModeBarline {
                visual_line,
                time: barline.time,
                scroll_speed: barline.scroll_speed,
            }
        })
        .collect()
}

/// Where on the screen a note should be drawn given the current time of the song, when the note
/// should be hit and how fast it travels.
pub fn x_position_of_note(current_time: f32, note_time: f32, scroll_speed: f32) -> f32 {
    NOTE_HIT_X + VELOCITY * (note_time - current_time) * scroll_speed
}

#[derive(Debug)]
enum VisualNote {
    Note(Sprite),
    Roll { start: Sprite, body: Shape, length: f32 },
}

#[derive(Debug)]
pub struct TaikoModeNote {
    visual_note: VisualNote,
    time: f32,
    scroll_speed: f32,
}

#[derive(Debug)]
pub struct TaikoModeBarline {
    visual_line: Shape,
    time: f32,
    scroll_speed: f32,
}

impl VisualNote {
    fn new(renderer: &Renderer, note: &Note, textures: &mut TextureCache) -> Option<Self> {
        let note_type = note.note_type;
        let pixel_vel = VELOCITY * note.scroll_speed;

        let mut get_texture = |filename| {
            textures
                .get(&renderer.device, &renderer.queue, filename)
                .unwrap()
        };
        let create_roll_body = |length: f32, height: f32| -> Result<Shape, TessellationError> {
            const OUTLINE_WIDTH: f32 = 3.;
            let dx = -height / 2.;
            let dy = -height / 2.;

            Ok(ShapeBuilder::new()
                .has_depth(true)
                // Outline
                .filled_rectangle(
                    [0., dy],
                    [length + dx, height + dy],
                    SolidColour::new([0., 0., 0., 1.]),
                )?
                .filled_circle(
                    [length + dx, 0.],
                    height / 2.,
                    SolidColour::new([0., 0., 0., 1.]),
                )?
                // Inside
                .filled_rectangle(
                    [OUTLINE_WIDTH, OUTLINE_WIDTH + dy],
                    [length - OUTLINE_WIDTH + dx, height - OUTLINE_WIDTH + dy],
                    SolidColour::new(ROLL_COLOUR),
                )?
                .filled_circle(
                    [length + dx, 0.],
                    height / 2. - OUTLINE_WIDTH,
                    SolidColour::new(ROLL_COLOUR),
                )?
                .build(&renderer.device))
        };

        let result = match note_type {
            NoteType::Don => {
                Self::Note(
                    SpriteBuilder::new(get_texture("don.png"))
                        .centre()
                        .depth(Some(0.))
                        .build(renderer)
                )
            }
            NoteType::Kat => {
                Self::Note(
                    SpriteBuilder::new(get_texture("kat.png"))
                        .centre()
                        .depth(Some(0.))
                        .build(renderer)
                )
            }
            NoteType::BigDon | NoteType::CoopDon => {
                Self::Note(
                    SpriteBuilder::new(get_texture("big_don.png"))
                        .centre()
                        .depth(Some(0.))
                        .build(renderer)
                )
            }
            NoteType::BigKat | NoteType::CoopKat => {
                Self::Note(
                    SpriteBuilder::new(get_texture("big_kat.png"))
                        .centre()
                        .depth(Some(0.))
                        .build(renderer)
                )
            }
                
            NoteType::Roll(length) => {
                let start = SpriteBuilder::new(get_texture("drumroll_start.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer);
                    
                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 100.0).ok()?;

                VisualNote::Roll { start, body, length: body_length }
            }

            NoteType::BigRoll(length) => {
                let start = SpriteBuilder::new(get_texture("big_drumroll_start.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer);

                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 150.0).ok()?;

                VisualNote::Roll { start, body, length: body_length }
            }

            NoteType::BalloonRoll(_, _) => {
                Self::Note(
                    SpriteBuilder::new(get_texture("balloon.png"))
                        .depth(Some(0.))
                        // The balloon texture is 300x100, but the notehead is centred at [50, 50].
                        .origin([50., 50.])
                        .build(renderer)
                )
            }

            _ => return None,
        };

        Some(result)
    }

    /// Sets the position of the note. The note will be centred at that position.
    fn set_position(&mut self, position: [f32; 2], depth: f32, renderer: &Renderer) {
        match self {
            VisualNote::Note(sprite) => {
                sprite.set_position(
                    position,
                    renderer,
                );

                sprite.set_depth(Some(depth), renderer);
            }

            VisualNote::Roll { start, body, .. } => {
                start.set_position(position, renderer);
                // TODO: do the same refactoring to shapes as I did to sprites
                body.set_position([position[0], position[1], depth], renderer);
            }
        }
    }

    fn set_position_for_time(
        &mut self,
        current_time: f32,
        note_time: f32,
        scroll_speed: f32,
        renderer: &Renderer,
    ) {
        self.set_position(
            [
                x_position_of_note(current_time, note_time, scroll_speed),
                NOTE_Y,
            ],
            note_time,
            renderer,
        );
    }
}

impl Renderable for VisualNote {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        match self {
            VisualNote::Note(sprite) => sprite.render(renderer, render_pass),
            VisualNote::Roll { start, body, .. } => {
                // If start and body both have the same depth, then start should render on top
                // of the body, given the compare function is `LessEqual`
                body.render(renderer, render_pass);
                start.render(renderer, render_pass);
            }
        }
    }
}

impl TaikoModeNote {
    pub fn new(renderer: &Renderer, note: &Note, textures: &mut TextureCache) -> Option<Self> {
        Some(Self {
            visual_note: VisualNote::new(renderer, note, textures)?,
            scroll_speed: note.scroll_speed,
            time: note.time,
        })
    }

    pub fn update_position(&mut self, renderer: &Renderer, note_adjusted_time: f32) {
        self.visual_note.set_position_for_time(
            note_adjusted_time,
            self.time,
            self.scroll_speed,
            renderer,
        )
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn scroll_speed(&self) -> f32 {
        self.scroll_speed
    }

    fn relative_bounding_box(&self) -> ([f32; 2], [f32; 2]) {
        match &self.visual_note {
            VisualNote::Note(sprite) => sprite.relative_bounding_box(),
            VisualNote::Roll { start, length, .. } => {
                let (head_start, head_fin) = start.relative_bounding_box();

                let start = head_start;
                let end = [head_fin[0] + *length, head_fin[1]];

                (start, end)
            }
        }
    }

    pub fn visible(&self, note_adjusted_time: f32) -> bool {
        let (rel_start, rel_end) = self.relative_bounding_box();
        let x_position = x_position_of_note(note_adjusted_time, self.time, self.scroll_speed);

        let start_x = rel_start[0] + x_position;
        let end_x = rel_end[0] + x_position;

        // TODO: seriously dont use hard coded resolution
        start_x < 1920. && end_x >= LEFT_PANEL_WIDTH
    }
}

impl TaikoModeBarline {
    pub fn update_position(&mut self, renderer: &Renderer, note_adjusted_time: f32) {
        self.visual_line.set_position(
            [
                x_position_of_note(note_adjusted_time, self.time, self.scroll_speed),
                NOTE_FIELD_Y,
                0.0,
            ],
            renderer,
        );
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn scroll_speed(&self) -> f32 {
        self.scroll_speed
    }
}

impl Renderable for TaikoModeNote {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.visual_note.render(renderer, render_pass);
    }
}

impl Renderable for TaikoModeBarline {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.visual_line.render(renderer, render_pass);
    }
}
