use std::time::Instant;

use kira::manager::AudioManager;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::tween::Tween;
use winit::event::VirtualKeyCode;

use crate::beatmap_parser::NoteTrack;
use crate::{
    beatmap_parser::Song,
    render::{
        shapes::{Shape, ShapeBuilder, SolidColour},
        texture::Sprite,
        Renderer,
    },
};
use crate::app::{Context, GameState, RenderContext, StateTransition, TextureCache};
use super::note::{create_visual_barlines, create_visual_notes, VisualNote};
use super::ui::{Header, NoteField};

pub struct TaikoMode {
    // UI Stuff
    background: Sprite,
    // TOOD: Give sprites a colour tint
    background_dim: Shape,
    header: Header,
    note_field: NoteField,

    // Song data
    song_handle: StaticSoundHandle,
    // The time of the song, with respect to the notes. Not with respect to the song itself, as
    // notes may be offset.
    started: bool,
    difficulty: usize,
    track: NoteTrack,

    temporary_start: Instant,

    // Sprites for the notes and barlines
    note_sprites: Vec<VisualNote>,
    barlines: Vec<Shape>,
}

impl TaikoMode {
    pub fn new(
        song: &Song,
        song_data: StaticSoundData,
        audio_manager: &mut AudioManager,
        difficulty: usize,
        renderer: &mut Renderer,
        textures: &mut TextureCache,
    ) -> anyhow::Result<Self> {
        let background = Sprite::new(
            textures.get(&renderer.device, &renderer.queue, "song_select_bg.jpg")?,
            [0.0; 3],
            &renderer.device,
            false,
        );

        let background_dim = ShapeBuilder::new()
            .filled_rectangle(
                [0., 0.],
                [1920., 1080.],
                SolidColour::new([0., 0., 0., 0.6]),
            )?
            .build(&renderer.device);

        let mut song_handle = audio_manager.play(song_data)?;
        // We want to start the song once the scene is actually loaded
        song_handle.pause(Tween::default())?;

        let track = 
            &song.difficulties[difficulty]
            .as_ref()
            .expect("Difficulty doesn't exist!")
            .track;

        Ok(Self {
            background,
            background_dim,
            header: Header::new(renderer, &song.title)?,
            note_field: NoteField::new(renderer)?,
            song_handle,
            started: false,
            difficulty,
            temporary_start: Instant::now(),
            // Possible performance problem: Cloning shouldn't be too big a deal but if the song is
            // really long it might become one
            track: track.clone(),
            note_sprites: create_visual_notes(renderer, textures, &track.notes),
            barlines: create_visual_barlines(renderer, &track.barlines),
        })
    }

    /// Returns what time it currently is with respect to the notes and global offset
    fn note_time(&self) -> f32 {
        self.temporary_start.elapsed().as_secs_f32()
    }
}

impl GameState for TaikoMode {
    fn update(&mut self, ctx: &mut Context, _delta_time: f32) -> StateTransition {
        if !self.started {
            self.song_handle.resume(Default::default()).unwrap();
            self.temporary_start = Instant::now();
            self.started = true;
        }

        if ctx.keyboard.is_pressed(VirtualKeyCode::Escape) {
            self.song_handle.stop(Default::default()).unwrap();
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }

    fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        ctx.render(&self.background);
        ctx.render(&self.background_dim);
        self.header.render(ctx);
        self.note_field.render(ctx);
    }
}

