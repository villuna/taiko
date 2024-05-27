use std::cmp::max;
use std::time::Instant;

use kira::manager::AudioManager;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::tween::Tween;
use winit::event::{ElementState, VirtualKeyCode, WindowEvent};

use super::note::{create_barlines, create_notes, EASY_NORMAL_TIMING, HARD_EXTREME_TIMING, NoteKeypressReaction, TaikoModeBarline, TaikoModeNote};
use super::ui::{Header, NoteField};
use crate::app::taiko_mode::note::x_position_of_note;
use crate::app::{Context, GameState, RenderContext, StateTransition, TextureCache};
use crate::render::texture::SpriteBuilder;
use crate::settings::{SETTINGS, settings};
use crate::{
    beatmap_parser::Song,
    render::{
        shapes::{Shape, ShapeBuilder, SolidColour},
        texture::Sprite,
        Renderer,
    },
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NoteJudgement {
    Bad,
    Ok,
    Good,
}

pub struct TaikoMode {
    // UI Stuff
    background: Sprite,
    // TODO: Give sprites a colour tint
    background_dim: Shape,
    header: Header,
    note_field: NoteField,

    /// A handle to the audio of the song
    song_handle: StaticSoundHandle,
    // Record the global offset, so we don't need to keep querying the settings
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
            // really long it might become one.
            notes: create_notes(renderer, textures, &track.notes),
            barlines: create_barlines(renderer, &track.barlines),
            next_note_index: 0,
            note_judgements: vec![],
            score: 0,
            soul_gauge: 0.0,
        })
    }

    /// Returns what time it is with respect to the notes and global offset.
    fn note_time(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32() - self.global_offset
    }

    /// Returns the timing windows to use for the song's difficulty.
    fn timing_windows(&self) -> &'static [f32; 3] {
        match self.difficulty {
            0 | 1 => &EASY_NORMAL_TIMING,
            _ => &HARD_EXTREME_TIMING,
        }
    }
}

impl GameState for TaikoMode {
    fn update(&mut self, ctx: &mut Context, _delta_time: f32) -> StateTransition {
        if !self.started {
            self.song_handle.resume(Default::default()).unwrap();
            self.started = true;
            self.start_time = Instant::now();
        }

        let time = self.note_time();
        // Advance our position in the list of notes as far as we can go
        while let Some(note) = self.notes.get(self.next_note_index) {
            if note.is_hittable(time, self.timing_windows()) {
                break;
            }

            self.next_note_index += 1;
        }

        if ctx.keyboard.is_pressed(VirtualKeyCode::Escape) {
            self.song_handle.stop(Default::default()).unwrap();
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }

    fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        // Update the positions of all the notes that are currently visible.
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
            // We handle the note input keyboard events the moment they are received for extra accuracy
            &WindowEvent::KeyboardInput { input, .. } => 'kbinput: {
                // TODO. Goodnight.
                let mut note_index = self.next_note_index;
                let Some(key) = input.virtual_keycode else {
                    break 'kbinput;
                };
                if settings().key_is_don_or_kat(key) && input.state == ElementState::Pressed {
                    let time = self.note_time();
                    dbg!(time);
                    let timing_windows = self.timing_windows();
                    loop {
                        // If there's no next note, we don't need to react.
                        let Some(mut next_note) = self.notes.get_mut(note_index) else {
                            println!("couldnt get a note at all");
                            break;
                        };

                        let reaction = next_note.receive_keypress(key, time, timing_windows);
                        dbg!(reaction, self.next_note_index);
                        match reaction {
                            // If it's the wrong colour, we'll keep checking to see if there's
                            // a note of the wright colour in scope.
                            NoteKeypressReaction::WrongColour => {}

                            NoteKeypressReaction::TooEarly => {
                                // Now we're only looking at notes that are unhittable, so stop here.
                                break;
                            }
                            NoteKeypressReaction::Hit(_) => {
                                self.next_note_index = note_index + 1;
                                // Ensure you only ever hit one note at a time
                                break;
                            }
                            NoteKeypressReaction::Drumroll { .. } => {
                                // TODO
                                println!("renda!");
                            }
                            NoteKeypressReaction::BalloonRoll { .. } => {
                                // TODO
                                println!("fuusen renda!")
                            }
                            NoteKeypressReaction::TooLate => {
                                // If this note is too late, we need to update our next note index,
                                // so we don't have to look through a bunch of unhittable notes.
                                self.next_note_index = note_index + 1;
                            }
                        }

                        note_index += 1;
                    }
                }
            }
            _ => {}
        }
    }
}
