//! Defines structs for drawing notes and barlines to the screen
use lyon::lyon_tessellation::TessellationError;
use winit::event::VirtualKeyCode;

use crate::app::taiko_mode::scene::NoteJudgement;
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

fn drumroll_visual_length(scroll_speed: f32, length_of_time: f32) -> f32 {
    scroll_speed * length_of_time * VELOCITY
}

/// The "Inner" taiko mode Note type is an enum containing data and behaviour specific to the note
/// type.
#[derive(Debug)]
enum NoteInner {
    Note {
        sprite: Sprite,
    },
    Roll {
        start_sprite: Sprite,
        body_sprite: Shape,
        length_of_time: f32,
    },
    Balloon {
        sprite: Sprite,
        hits_left: u32,
        length_of_time: f32,
    },
}

#[derive(Debug)]
pub struct TaikoModeNote {
    note: NoteInner,
    time: f32,
    scroll_speed: f32,
    /// Whether to display the note or not (regardless of its position on the screen).
    /// E.g., notes that have already been hit should not be displayed.
    on_screen: bool,
}

#[derive(Debug)]
pub struct TaikoModeBarline {
    visual_line: Shape,
    time: f32,
    scroll_speed: f32,
}

impl NoteInner {
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
            NoteType::Don => Self::Note {
                sprite: SpriteBuilder::new(get_texture("don.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer),
            },
            NoteType::Kat => Self::Note {
                sprite: SpriteBuilder::new(get_texture("kat.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer),
            },
            NoteType::BigDon | NoteType::CoopDon => Self::Note {
                sprite: SpriteBuilder::new(get_texture("big_don.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer),
            },
            NoteType::BigKat | NoteType::CoopKat => Self::Note {
                sprite: SpriteBuilder::new(get_texture("big_kat.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer),
            },

            NoteType::Roll(length) => {
                let start = SpriteBuilder::new(get_texture("drumroll_start.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer);

                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 100.0).ok()?;

                NoteInner::Roll {
                    start_sprite: start,
                    body_sprite: body,
                    length_of_time: length,
                }
            }

            NoteType::BigRoll(length) => {
                let start = SpriteBuilder::new(get_texture("big_drumroll_start.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer);

                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 150.0).ok()?;

                NoteInner::Roll {
                    start_sprite: start,
                    body_sprite: body,
                    length_of_time: length,
                }
            }

            NoteType::BalloonRoll(_, _) => {
                Self::Note {
                    sprite: SpriteBuilder::new(get_texture("balloon.png"))
                        .depth(Some(0.))
                        // The balloon texture is 300x100, but the notehead is centred at [50, 50].
                        .origin([50., 50.])
                        .build(renderer),
                }
            }

            _ => return None,
        };

        Some(result)
    }

    /// Sets the position of the note. The note will be centred at that position.
    fn set_position(&mut self, position: [f32; 2], depth: f32, renderer: &Renderer) {
        match self {
            NoteInner::Note { sprite } => {
                sprite.set_position(position, renderer);
                sprite.set_depth(Some(depth), renderer);
            }

            NoteInner::Balloon { sprite, .. } => {
                sprite.set_position(position, renderer);
                sprite.set_depth(Some(depth), renderer);
            }

            NoteInner::Roll {
                start_sprite: start,
                body_sprite: body,
                ..
            } => {
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

impl Renderable for NoteInner {
    fn render<'pass>(
        &'pass self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        match self {
            NoteInner::Note { sprite } => sprite.render(renderer, render_pass),
            NoteInner::Balloon { sprite, .. } => sprite.render(renderer, render_pass),
            NoteInner::Roll {
                start_sprite: start,
                body_sprite: body,
                ..
            } => {
                // If start and body both have the same depth, then start should render on top
                // of the body, given the compare function is `LessEqual`
                body.render(renderer, render_pass);
                start.render(renderer, render_pass);
            }
        }
    }
}

/// Different ways a note can respond to a keypress
/// See [TaikoModeNote::receive_input]
pub enum NoteInputReaction {
    /// Don was pressed but this note is Kat, or vice versa
    /// Basically, do absolutely nothing.
    WrongColour,
    /// The keypress is too early, so the note is not yet able to be hit
    ///
    /// *This variant is more important than WrongColour*. If a keypress is both too early and the
    /// wrong colour, this is the one you should return, since the calling code uses this variant
    /// to determine where to stop calling [TaikoModeNote::receive_input]
    TooEarly,
    /// The note was hit, with the given result
    Hit(NoteJudgement),
    /// The note was hit, and is a drumroll.
    /// Since drumrolls can be big or small, and can be hit with either don or kat, we return the
    /// type of note so that we can display the correct flying note
    Drumroll { roll_note: NoteType },
    /// The note was hit, and is a balloon.
    /// These notes do different things on the first hit and the last hit, so this info is
    /// returned as well.
    BalloonRoll { first: bool, popped: bool },
    /// The note cannot be hit anymore.
    TooLate,
}

impl TaikoModeNote {
    pub fn new(renderer: &Renderer, note: &Note, textures: &mut TextureCache) -> Option<Self> {
        Some(Self {
            note: NoteInner::new(renderer, note, textures)?,
            scroll_speed: note.scroll_speed,
            time: note.time,
            on_screen: true,
        })
    }

    pub fn update_position(&mut self, renderer: &Renderer, note_adjusted_time: f32) {
        self.note
            .set_position_for_time(note_adjusted_time, self.time, self.scroll_speed, renderer)
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn scroll_speed(&self) -> f32 {
        self.scroll_speed
    }

    pub fn visible(&self, note_adjusted_time: f32) -> bool {
        if !self.on_screen {
            return false;
        }

        let (rel_start, rel_end) = self.relative_bounding_box();
        let x_position = x_position_of_note(note_adjusted_time, self.time, self.scroll_speed);

        let start_x = rel_start[0] + x_position;
        let end_x = rel_end[0] + x_position;

        // TODO: seriously dont use hard coded resolution
        start_x < 1920. && end_x >= LEFT_PANEL_WIDTH
    }

    /// Reacts to a keypress.
    pub fn receive_keypress(
        &mut self,
        key: VirtualKeyCode,
        note_adjusted_time: f32,
    ) -> NoteInputReaction {
        todo!()
    }

    /// Returns whether the note is (or will at some point be) hittable
    ///
    /// When checking if a note has been hit by the player, we start checking from the first
    /// hittable note. If the note can be hit now or at some point in the future, it is considered
    /// "hittable". If it is past its time, however, it is not hittable.
    pub fn is_hittable(&self, time: f32, timing_windows: [f32; 3]) -> bool {
        match self.note {
            NoteInner::Note { .. } => todo!(),
            NoteInner::Roll { .. } => todo!(),
            NoteInner::Balloon { .. } => todo!(),
        }
    }

    fn relative_bounding_box(&self) -> ([f32; 2], [f32; 2]) {
        match &self.note {
            NoteInner::Note { sprite } => sprite.relative_bounding_box(),
            NoteInner::Balloon { sprite, .. } => sprite.relative_bounding_box(),
            NoteInner::Roll {
                start_sprite,
                length_of_time,
                ..
            } => {
                let (head_start, head_fin) = start_sprite.relative_bounding_box();

                let start = head_start;
                let end = [
                    head_fin[0] + drumroll_visual_length(self.scroll_speed, *length_of_time),
                    head_fin[1],
                ];

                (start, end)
            }
        }
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
        self.note.render(renderer, render_pass);
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
