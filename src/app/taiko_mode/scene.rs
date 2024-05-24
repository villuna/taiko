use std::time::Instant;

use kira::manager::AudioManager;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::tween::Tween;
use winit::event::{VirtualKeyCode, WindowEvent};

use super::note::{create_barlines, create_notes, TaikoModeBarline, TaikoModeNote};
use super::ui::{Header, NoteField};
use crate::app::taiko_mode::note::x_position_of_note;
use crate::app::{Context, GameState, RenderContext, StateTransition, TextureCache};
use crate::render::texture::SpriteBuilder;
use crate::settings::SETTINGS;
use crate::{
    beatmap_parser::Song,
    render::{
        shapes::{Shape, ShapeBuilder, SolidColour},
        texture::Sprite,
        Renderer,
    },
};

pub enum NoteJudgement {
    Bad,
    Ok,
    Good,
}
pub struct TaikoMode {
    // UI Stuff
    background: Sprite,
    // TOOD: Give sprites a colour tint
    background_dim: Shape,
    header: Header,
    note_field: NoteField,

    /// A handle to the audio of the song
    song_handle: StaticSoundHandle,
    // Record the global offset so we don't need to keep querying the settings
    // This is fine bc the settings will never change mid-song but if that's ever possible, we'd
    // need to update this every time the setting changed.
    global_offset: f32,

    // The instant the song started.
    // Even though the song handle keeps track of the position through the song, that value is
    // choppy and using it for the position of the notes will cause the notes to stutter. So we
    // need to keep track of the time ourselves.
    start_time: Instant,
    started: bool,
    difficulty: usize,

    notes: Vec<TaikoModeNote>,
    barlines: Vec<TaikoModeBarline>,

    // Scoring stuff
    /// the index of the next note to be played
    next_note_index: usize,
    note_judgements: Vec<NoteJudgement>,
    score: usize,
    /// The percentage the soul gauge is filled
    soul_gauge: f32,
    // TODO: A UI element containing text displaying the judgement
    // "Good" "Ok" "Bad" (and ideally in japanese too, "良", "可", "不可")
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
        let bg_texture = textures.get(&renderer.device, &renderer.queue, "song_select_bg.jpg")?;
        let background = SpriteBuilder::new(bg_texture).build(renderer);

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

        let track = &song.difficulties[difficulty]
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
            start_time: Instant::now(),
            global_offset: SETTINGS.read().unwrap().game.global_note_offset / 1000.0,
            difficulty,
            // Possible performance problem: Cloning shouldn't be too big a deal but if the song is
            // really long it might become one
            notes: create_notes(renderer, textures, &track.notes),
            barlines: create_barlines(renderer, &track.barlines),
            next_note_index: 0,
            note_judgements: vec![],
            score: 0,
            soul_gauge: 0.0,
        })
    }

    /// Returns what time it currently is with respect to the notes and global offset
    fn note_time(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32() - self.global_offset
    }
}

impl GameState for TaikoMode {
    fn update(&mut self, ctx: &mut Context, _delta_time: f32) -> StateTransition {
        if !self.started {
            self.song_handle.resume(Default::default()).unwrap();
            self.started = true;
            self.start_time = Instant::now();
        }

        if ctx.keyboard.is_pressed(VirtualKeyCode::Escape) {
            self.song_handle.stop(Default::default()).unwrap();
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }

    fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        // Update the positions of all the notes that are currently visible
        let time = self.note_time();

        let on_screen_notes = self.notes.iter_mut().filter(|note| note.visible(time));

        for note in on_screen_notes {
            note.update_position(ctx.renderer, time);
        }

        let on_screen_barlines = self.barlines.iter_mut().filter(|barline| {
            let pos = x_position_of_note(time, barline.time(), barline.scroll_speed());
            pos >= 0. && pos <= 1920.
        });

        for barline in on_screen_barlines {
            barline.update_position(ctx.renderer, time);
        }

        ctx.render(&self.background);
        ctx.render(&self.background_dim);
        self.header.render(ctx);

        let notes = self.notes.iter().filter(|note| note.visible(time));

        let barlines = self.barlines.iter().filter(|barline| {
            let pos = x_position_of_note(time, barline.time(), barline.scroll_speed());
            // TODO: another hardcoded resolution to get rid of
            pos >= 0. && pos <= 1920.
        });

        self.note_field.render(ctx, notes, barlines);
    }

    fn handle_event(&mut self, _ctx: &mut Context, event: &WindowEvent<'_>) {
        match event {
            // Handle keyboard input events the moment they are received for extra accuracy
            &WindowEvent::KeyboardInput { input, .. } => 'kbinput: {
                // If there is no next note, we don't need to react to note keyboard input
                let Some(next_note) = self.notes.get(self.next_note_index) else {
                    break 'kbinput;
                };
            }
            _ => {}
        }
    }
}
