use lyon::lyon_tessellation::TessellationError;

use crate::beatmap_parser::track::NoteType;
use silkwood::{app::TextureCache, render::shapes::ShapeBuilder};

use silkwood::render::{
    context::Renderable,
    shapes::{Shape, SolidColour},
    texture::Sprite,
    RenderPassContext,
};

const ROLL_COLOUR: [f32; 4] = [1.0, 195.0 / 255.0, 44.0 / 255.0, 1.0];

#[derive(Debug)]
pub enum VisualNote {
    Note(Sprite),
    Roll { start: Sprite, body: Shape },
}

impl VisualNote {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        note_type: &NoteType,
        pixel_vel: f32,
        textures: &mut TextureCache,
    ) -> Option<Self> {
        let mut get_texture = |filename| textures.get(device, queue, filename).unwrap();
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
                .build(device))
        };

        Some(match note_type {
            NoteType::Don => {
                Self::Note(Sprite::new(get_texture("don.png"), [0.0; 3], device, true))
            }
            NoteType::Kat => {
                Self::Note(Sprite::new(get_texture("kat.png"), [0.0; 3], device, true))
            }
            NoteType::BigDon | NoteType::CoopDon => Self::Note(Sprite::new(
                get_texture("big_don.png"),
                [0.0; 3],
                device,
                true,
            )),
            NoteType::BigKat | NoteType::CoopKat => Self::Note(Sprite::new(
                get_texture("big_kat.png"),
                [0.0; 3],
                device,
                true,
            )),
            NoteType::Roll(length) => {
                let start = Sprite::new(get_texture("drumroll_start.png"), [0.0; 3], device, true);
                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 100.0).ok()?;

                VisualNote::Roll { start, body }
            }

            NoteType::BigRoll(length) => {
                let start = Sprite::new(
                    get_texture("big_drumroll_start.png"),
                    [0.0; 3],
                    device,
                    true,
                );
                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 150.0).ok()?;

                VisualNote::Roll { start, body }
            }

            _ => return None,
        })
    }

    /// Sets the position of the note.
    ///
    /// The note will be centered at this position, that is to say
    /// if the note is set to be at the position where it should be
    pub fn set_position(&mut self, position: [f32; 3], queue: &wgpu::Queue) {
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
}

impl Renderable for VisualNote {
    fn render<'a>(&'a self, ctx: &mut RenderPassContext<'a>) {
        match self {
            VisualNote::Note(sprite) => sprite.render(ctx),
            VisualNote::Roll { start, body } => {
                // If start and body both have the same depth, then start should render on top
                // of the body, given the compare function is `LessEqual`
                body.render(ctx);
                start.render(ctx);
            }
        }
    }
}
