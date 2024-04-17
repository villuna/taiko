//! Defines structs for drawing notes and barlines to the screen
use lyon::lyon_tessellation::TessellationError;

use crate::beatmap_parser::track::NoteType;
use crate::beatmap_parser::{Barline, Note};
use crate::render::Renderer;
use crate::{app::TextureCache, render::shapes::ShapeBuilder};

use crate::render::{
    Renderable,
    shapes::{Shape, SolidColour},
    texture::Sprite,
};

use super::ui::{NOTE_FIELD_HEIGHT, NOTE_FIELD_Y, NOTE_HIT_X, NOTE_Y};

const VELOCITY: f32 = (1920. - NOTE_HIT_X) / 2.;
const ROLL_COLOUR: [f32; 4] = [1., 195. / 255., 44. / 255., 1.];

/// Takes a list of notes in a song and creates visual representations for all of them.
pub fn create_visual_notes(renderer: &mut Renderer, textures: &mut TextureCache, notes: &[Note]) -> Vec<VisualNote> {
    notes.iter()
        .filter_map(|note|
            VisualNote::new(renderer, note.note_type, VELOCITY * note.scroll_speed, textures)
        )
        .collect()
}

/// Takes a list of barlines in a song and creates visual representations for all of them.
pub fn create_visual_barlines(renderer: &mut Renderer, barlines: &[Barline]) -> Vec<Shape> {
    barlines.iter()
        .filter_map(|barline| {
            Some(ShapeBuilder::new()
                .filled_rectangle([-1., 0.], [1., NOTE_FIELD_HEIGHT], SolidColour::new([1., 1., 1., 0.5])).ok()?
                .position([x_position_of_note(barline.time, 0., barline.scroll_speed), NOTE_FIELD_Y, 0.])
                .build(&renderer.device))
        })
        .collect()
}

/// Where on the screen a note should be drawn given the current time of the song, when the note
/// should be hit and how fast it travels.
pub fn x_position_of_note(current_time: f32, note_time: f32, scroll_speed: f32) -> f32 {
    NOTE_HIT_X + VELOCITY * (note_time - current_time) * scroll_speed
}

#[derive(Debug)]
pub enum VisualNote {
    Note(Sprite),
    Roll { start: Sprite, body: Shape },
}

impl VisualNote {
    pub fn new(
        renderer: &mut Renderer,
        note_type: NoteType,
        pixel_vel: f32,
        textures: &mut TextureCache,
    ) -> Option<Self> {
        let mut get_texture = |filename| textures.get(&renderer.device, &renderer.queue, filename).unwrap();
        let create_roll_body = |length, height| -> Result<Shape, TessellationError> {
            const OUTLINE_WIDTH: f32 = 3.0;

            Ok(ShapeBuilder::new()
                .has_depth(true)
                // Outline
                .filled_rectangle(
                    [height / 2.0, 0.0],
                    [length, height],
                    SolidColour::new([0.0, 0.0, 0.0, 1.0]),
                )?
                .filled_circle(
                    [length, height / 2.0],
                    height / 2.0,
                    SolidColour::new([0.0, 0.0, 0.0, 1.0]),
                )?
                // Inside
                .filled_rectangle(
                    [height / 2.0 + OUTLINE_WIDTH, OUTLINE_WIDTH],
                    [length - OUTLINE_WIDTH, height - OUTLINE_WIDTH],
                    SolidColour::new(ROLL_COLOUR),
                )?
                .filled_circle(
                    [length, height / 2.0],
                    height / 2.0 - OUTLINE_WIDTH,
                    SolidColour::new(ROLL_COLOUR),
                )?
                .build(&renderer.device))
        };

        // Not sure i like this code style, even if it does cut down on code reuse
        // maybe refactor?
        Some(match note_type {
            NoteType::Don => {
                Self::Note(Sprite::new(get_texture("don.png"), [0.0; 3], &renderer.device, true))
            }
            NoteType::Kat => {
                Self::Note(Sprite::new(get_texture("kat.png"), [0.0; 3], &renderer.device, true))
            }
            NoteType::BigDon | NoteType::CoopDon => Self::Note(Sprite::new(
                get_texture("big_don.png"),
                [0.0; 3],
                &renderer.device,
                true,
            )),
            NoteType::BigKat | NoteType::CoopKat => Self::Note(Sprite::new(
                get_texture("big_kat.png"),
                [0.0; 3],
                &renderer.device,
                true,
            )),
            NoteType::Roll(length) => {
                let start = Sprite::new(get_texture("drumroll_start.png"), [0.0; 3], &renderer.device, true);
                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 100.0).ok()?;

                VisualNote::Roll { start, body }
            }

            NoteType::BigRoll(length) => {
                let start = Sprite::new(
                    get_texture("big_drumroll_start.png"),
                    [0.0; 3],
                    &renderer.device,
                    true,
                );
                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 150.0).ok()?;

                VisualNote::Roll { start, body }
            }

            NoteType::BalloonRoll(_, _) => Self::Note(Sprite::new(
                get_texture("balloon.png"),
                // The balloon texture is 300x100.
                // the notehead is at [50, 50]
                // so this is the offset we need to move the centre to the centre of the notehead
                [100.0, 0.0, 0.0],
                &renderer.device,
                true,
            )),

            _ => return None,
        })
    }

    /// Sets the position of the note.
    ///
    /// The note will be centered at this position, that is to say
    /// if the note is set to be at the position where it should be
    fn set_position(&mut self, position: [f32; 3], queue: &wgpu::Queue) {
        match self {
            VisualNote::Note(sprite) => {
                let (x, y) = sprite.dimensions();
                let (x_offset, y_offset) = (x as f32 / 2.0, y as f32 / 2.0);

                sprite.set_position(
                    [position[0] - x_offset, position[1] - y_offset, position[2]],
                    queue,
                );
            }

            VisualNote::Roll { start, body } => {
                let (x, y) = start.dimensions();
                let (x_offset, y_offset) = (x as f32 / 2.0, y as f32 / 2.0);

                let new_position = [position[0] - x_offset, position[1] - y_offset, position[2]];

                start.set_position(new_position, queue);
                body.set_position(new_position, queue);
            }
        }
    }

    pub fn set_position_for_time(&mut self, renderer: &mut Renderer, current_time: f32, note_time: f32, scroll_speed: f32) {
        self.set_position([x_position_of_note(current_time, note_time, scroll_speed), NOTE_Y, note_time], &renderer.queue);
    }
}

impl Renderable for VisualNote {
    fn render<'pass>(&'pass self, renderer: &'pass Renderer, render_pass: &mut wgpu::RenderPass<'pass>) {
        match self {
            VisualNote::Note(sprite) => sprite.render(renderer, render_pass),
            VisualNote::Roll { start, body } => {
                // If start and body both have the same depth, then start should render on top
                // of the body, given the compare function is `LessEqual`
                body.render(renderer, render_pass);
                start.render(renderer, render_pass);
            }
        }
    }
}
