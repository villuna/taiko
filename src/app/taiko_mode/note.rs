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
use crate::settings::{SETTINGS, settings};

use super::ui::{LEFT_PANEL_WIDTH, NOTE_FIELD_HEIGHT, NOTE_FIELD_Y, NOTE_HIT_X, NOTE_Y};

const VELOCITY: f32 = (1920. - NOTE_HIT_X) / 2.;
const ROLL_COLOUR: [f32; 4] = [1., 195. / 255., 44. / 255., 1.];

// Nice expressive aliases for the indices we'll use for note judgements
const GOOD: usize = 0;
const OK: usize = 1;
const BAD: usize = 2;

// I have to credit OpenTaiko as that's where I got these values.
// (and also for inspiring me to give making my own simulator a red-hot go)
pub const EASY_NORMAL_TIMING: [f32; 3] = [0.042, 0.108, 0.125];
pub const HARD_EXTREME_TIMING: [f32; 3] = [0.025, 0.075, 0.108];

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

// I wonder if these two types could fit into the parser module
// They're obviously pretty important but, it seems they're not that useful in the parser module
// itself, since that module has the more general NoteType enum.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum NoteColour {
    Don,
    Kat,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct BasicNoteType {
    colour: NoteColour,
    big: bool,
}

impl BasicNoteType {
    fn is_hit_by(&self, key: VirtualKeyCode) -> bool {
        let settings = SETTINGS.read().unwrap();
        match self.colour {
            NoteColour::Don => settings.key_is_don(key),
            NoteColour::Kat => settings.key_is_kat(key),
        }
    }
}

impl TryFrom<NoteType> for BasicNoteType {
    type Error = ();

    fn try_from(value: NoteType) -> Result<Self, Self::Error> {
        match value {
            NoteType::Don => Ok(Self {
                colour: NoteColour::Don,
                big: false,
            }),
            NoteType::Kat => Ok(Self {
                colour: NoteColour::Kat,
                big: false,
            }),
            NoteType::BigDon => Ok(Self {
                colour: NoteColour::Don,
                big: true,
            }),
            NoteType::BigKat => Ok(Self {
                colour: NoteColour::Kat,
                big: true,
            }),
            _ => Err(()),
        }
    }
}

/// The "Inner" taiko mode Note type is an enum containing data and behaviour specific to the note
/// type.
#[derive(Debug)]
pub(crate) enum NoteInner {
    Note {
        sprite: Sprite,
        kind: BasicNoteType,
        is_hit: bool,
    },
    Roll {
        start_sprite: Sprite,
        body_sprite: Shape,
        big: bool,
        duration: f32,
    },
    Balloon {
        sprite: Sprite,
        hits_left: u32,
        duration: f32,
    },
}

#[derive(Debug)]
pub struct TaikoModeNote {
    pub(crate) note: NoteInner,
    time: f32,
    scroll_speed: f32,
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
            NoteType::Don
            | NoteType::Kat
            | NoteType::BigDon
            | NoteType::CoopDon
            | NoteType::BigKat
            | NoteType::CoopKat => {
                let sprite_name = match note_type {
                    NoteType::Don => "don.png",
                    NoteType::Kat => "kat.png",
                    NoteType::BigDon | NoteType::CoopDon => "big_don.png",
                    NoteType::BigKat | NoteType::CoopKat => "big_don.png",
                    _ => unreachable!(),
                };

                Self::Note {
                    sprite: SpriteBuilder::new(get_texture(sprite_name))
                        .centre()
                        .depth(Some(0.))
                        .build(renderer),
                    kind: note_type.try_into().unwrap(),
                    is_hit: false,
                }
            }

            NoteType::Roll(length) | NoteType::BigRoll(length) => {
                let start = SpriteBuilder::new(get_texture("drumroll_start.png"))
                    .centre()
                    .depth(Some(0.))
                    .build(renderer);

                let body_length = pixel_vel * length;
                let body = create_roll_body(body_length, 100.0).ok()?;

                NoteInner::Roll {
                    start_sprite: start,
                    body_sprite: body,
                    duration: length,
                    big: matches!(note_type, NoteType::BigRoll(_)),
                }
            }

            NoteType::BalloonRoll(duration, hits_left) => {
                Self::Balloon {
                    sprite: SpriteBuilder::new(get_texture("balloon.png"))
                        .depth(Some(0.))
                        // The balloon texture is 300x100, but the notehead is centred at [50, 50].
                        .origin([50., 50.])
                        .build(renderer),
                    hits_left,
                    duration,
                }
            }

            _ => return None,
        };

        Some(result)
    }

    /// Sets the position of the note. The note will be centred at that position.
    fn set_position(&mut self, position: [f32; 2], depth: f32, renderer: &Renderer) {
        match self {
            NoteInner::Note { sprite, .. } => {
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
        // TODO: Specialise this so that balloons do their weird behaviour
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
            NoteInner::Note { sprite, is_hit, .. } => {
                if !is_hit {
                    sprite.render(renderer, render_pass)
                }
            }
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
/// See [TaikoModeNote::receive_keypress]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NoteKeypressReaction {
    /// Don was pressed but this note is Kat, or vice versa
    /// Basically, do absolutely nothing.
    WrongColour,
    /// The keypress is too early, so the note is not yet able to be hit.
    ///
    /// *This variant is more important than WrongColour*. If a keypress is both too early and the
    /// wrong colour, this is the one you should return, since the calling code uses this variant
    /// to determine where to stop calling [TaikoModeNote::receive_keypress]
    TooEarly,
    /// The note was hit, with the given result
    Hit(NoteJudgement),
    /// The note was hit, and is a drumroll.
    /// Since drumrolls can be big or small, and can be hit with either don or kat, we return the
    /// note type so that we can display the correct flying note.
    Drumroll { roll_note: BasicNoteType },
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
        if let NoteInner::Note { is_hit, .. } = self.note {
            if is_hit {
                return false;
            }
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
        time: f32,
        timing_windows: &[f32; 3],
    ) -> NoteKeypressReaction {
        // Before this function was called, we should have checked that the keypress is actually
        // don or kat.
        if !settings().key_is_don_or_kat(key) {
            panic!(
                "Keycode was passed to TaikoModeNote::receive_keypress that was not don or kat."
            );
        }

        if !self.is_hittable(time, timing_windows) {
            return NoteKeypressReaction::TooLate;
        }

        match &mut self.note {
            NoteInner::Note { kind, is_hit, .. } => {
                // If the earliest the note could ever be hit is later (greater than) the current
                // time, then we are too early.
                if self.time - timing_windows[BAD] > time {
                    NoteKeypressReaction::TooEarly
                } else if kind.is_hit_by(key) {
                    // Otherwise we check to see exactly how close the times are.
                    if (time - self.time).abs() < timing_windows[GOOD] {
                        *is_hit = true;
                        NoteKeypressReaction::Hit(NoteJudgement::Good)
                    } else if (time - self.time).abs() < timing_windows[OK] {
                        *is_hit = true;
                        NoteKeypressReaction::Hit(NoteJudgement::Ok)
                    } else {
                        *is_hit = true;
                        NoteKeypressReaction::Hit(NoteJudgement::Bad)
                    }
                } else {
                    NoteKeypressReaction::WrongColour
                }
            }

            NoteInner::Roll { duration, big, .. } => {
                let relative_time = time - self.time;
                if relative_time < 0.0 {
                    // This is before the drumroll
                    NoteKeypressReaction::TooEarly
                } else if relative_time >= *duration {
                    // This is after
                    NoteKeypressReaction::TooLate
                } else {
                    // This is just right
                    let settings = SETTINGS.read().unwrap();
                    let colour = if settings.key_is_don(key) {
                        NoteColour::Don
                    } else {
                        // We already checked that the key was either don or kat.
                        NoteColour::Kat
                    };

                    let roll_note = BasicNoteType { colour, big: *big };

                    NoteKeypressReaction::Drumroll { roll_note }
                }
            }

            NoteInner::Balloon { duration, .. } => {
                // TODO: Finish this implementation.
                // Goodnight!
                if self.time > time {
                    NoteKeypressReaction::TooEarly
                } else if self.time + *duration < time {
                    NoteKeypressReaction::TooLate
                } else {
                    NoteKeypressReaction::WrongColour
                }
            }
        }
    }

    /// Whether the note is (or will at some point be) hittable.
    ///
    /// When checking if a note has been hit by the player, we start checking from the first
    /// hittable note. If the note can be hit now or at some point in the future, it is considered
    /// "hittable". If it is past its time, however, it is not hittable.
    pub fn is_hittable(&self, time: f32, timing_windows: &[f32; 3]) -> bool {
        match self.note {
            NoteInner::Note { is_hit, .. } => {
                // If the note is hit, obviously it won't be hittable again.
                // If the latest the note could ever be hit is later than the current time, then
                // there's still a chance it's hittable.
                !is_hit && self.time + timing_windows[BAD] > time
            }
            NoteInner::Roll { duration, .. } => self.time + duration > time,
            NoteInner::Balloon {
                duration,
                hits_left,
                ..
            } => hits_left > 0 && self.time + duration > time,
        }
    }

    fn relative_bounding_box(&self) -> ([f32; 2], [f32; 2]) {
        match &self.note {
            NoteInner::Note { sprite, .. } => sprite.relative_bounding_box(),
            NoteInner::Balloon { sprite, .. } => sprite.relative_bounding_box(),
            NoteInner::Roll {
                start_sprite,
                duration: length_of_time,
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
