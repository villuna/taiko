use std::time::Instant;

use kira::manager::AudioManager;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::tween::Tween;
use winit::event::{ElementState, VirtualKeyCode, WindowEvent};

use super::note::{create_barlines, create_notes, NoteKeypressReaction, TaikoModeBarline, TaikoModeNote, BAD, EASY_NORMAL_TIMING, GOOD, HARD_EXTREME_TIMING, OK, NoteInner};
use super::ui::{BalloonDisplay, Header, JudgementText, NoteField};
use crate::app::taiko_mode::note::x_position_of_note;
use crate::app::{Context, GameState, RenderContext, StateTransition, TextureCache};
use crate::render::texture::{
    AnimatedSprite, AnimatedSpriteBuilder, Frame, PlaybackState, SpriteBuilder,
};
use crate::settings::{settings, SETTINGS};
use crate::{
    beatmap_parser::Song,
    render::{
        shapes::{Shape, ShapeBuilder, SolidColour},
        texture::Sprite,
        Renderer,
    },
};

pub type ScoreInt = u64;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NoteJudgement {
    Bad,
    Ok,
    Good,
}

impl NoteJudgement {
    fn from_offset(offset: f32, timing_windows: &[f32; 3]) -> Option<Self> {
        let abs_offset = offset.abs();
        if abs_offset < timing_windows[GOOD] {
            Some(Self::Good)
        } else if abs_offset < timing_windows[OK] {
            Some(Self::Ok)
        } else if abs_offset < timing_windows[BAD] {
            Some(Self::Bad)
        } else {
            None
        }
    }
}

impl NoteJudgement {
    pub fn index(&self) -> usize {
        match self {
            NoteJudgement::Bad => BAD,
            NoteJudgement::Ok => OK,
            NoteJudgement::Good => GOOD,
        }
    }
}

/// A record containing statistics about how the player has done.
///
/// This struct will slowly collate data as the game progresses, and will be passed to the score
/// screen at the end.
///
/// Contains more information than is usually collected in taiko games. I want this sim to be able
/// to display a bunch of interesting gameplay statistics, and all that will be stored here.
#[derive(Clone, Default, Debug)]
pub struct PlayResult {
    /// A vector containing the judgements for every note recorded.
    /// A None value indicates a miss.
    judgements: Vec<Option<NoteJudgement>>,
    drumrolls: u64,
    score: ScoreInt,
    /// For all the notes that were hit (good, okay, or bad), records the difference between when
    /// the note was hit and when the note should have been hit.
    hit_errors: Vec<f32>,
}

impl PlayResult {
    fn new() -> Self {
        Self::default()
    }
}

pub struct TaikoMode {
    // UI Stuff
    background: Sprite,
    // TODO: Give sprites a colour tint
    background_dim: Shape,
    header: Header,
    note_field: NoteField,
    balloon_display: BalloonDisplay,

    /// A handle to the audio of the song
    song_handle: StaticSoundHandle,
    // Record the global offset, so we don't need to keep querying the settings
    // This is fine bc the settings will never change mid-song but if that's ever possible, we'd
    // need to update this every time the setting changed.
    global_offset: f32,

    /// The instant the song started.
    ///
    /// Even though the song handle keeps track of the position through the song, that value is
    /// choppy and using it for the position of the notes will cause the notes to stutter. So we
    /// need to keep track of the time ourselves.
    start_time: Instant,
    started: bool,
    difficulty: usize,

    notes: Vec<TaikoModeNote>,
    barlines: Vec<TaikoModeBarline>,

    // Note scoring/input handling
    /// The index of the next note to be played
    next_note_index: usize,
    /// The percentage the soul gauge is filled
    soul_gauge: f32,
    note_judgement_text: JudgementText,

    /// An ongoing record of the player's performance.
    /// At the end of the song, this will be passed to the score screen.
    results: PlayResult,
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
            balloon_display: BalloonDisplay::new(textures, renderer)?,
            song_handle,
            started: false,
            start_time: Instant::now(),
            global_offset: SETTINGS.read().unwrap().game.global_note_offset / 1000.0,
            difficulty,
            notes: create_notes(renderer, textures, &track.notes),
            barlines: create_barlines(renderer, &track.barlines),
            next_note_index: 0,
            soul_gauge: 0.0,
            note_judgement_text: JudgementText::new(renderer),
            results: PlayResult::new(),
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

    /// Considers the next note to have been missed. Updates the index of the next note, and adds a
    /// miss to the play result if appropriate.
    fn skip_next_note(&mut self) {
        if let Some(note) = self.notes.get(self.next_note_index) {
            self.next_note_index += 1;

            if note.is_don_or_kat() {
                self.results.judgements.push(None);
            } else if matches!(note.note, NoteInner::Balloon { .. }) {
                self.balloon_display.discard();
            }
        }
    }
}

impl GameState for TaikoMode {
    fn update(&mut self, ctx: &mut Context, delta_time: f32) -> StateTransition {
        if !self.started {
            self.song_handle.resume(Default::default()).unwrap();
            self.started = true;
            self.start_time = Instant::now();
        }

        self.note_judgement_text.update(ctx.renderer);
        self.balloon_display.update(delta_time);

        let time = self.note_time();
        // Advance our position in the list of notes as far as we can go
        while let Some(note) = self.notes.get(self.next_note_index) {
            if note.is_hittable(time, self.timing_windows()) {
                break;
            }

            self.skip_next_note();
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
            (0.0..1920.0).contains(&pos)
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
            (0.0..190.0).contains(&pos)
        });

        self.note_field.render(ctx, notes, barlines);
        ctx.render(&self.note_judgement_text);
        ctx.render(&self.balloon_display);
    }

    fn handle_event(&mut self, ctx: &mut Context, event: &WindowEvent<'_>) {
        // We handle the note input keyboard events the moment they are received for extra accuracy
        if let &WindowEvent::KeyboardInput { input, .. } = event {
            let mut note_index = self.next_note_index;
            let Some(key) = input.virtual_keycode else {
                return;
            };

            // Keys have this annoying tendency to repeat presses when held down,
            // so we gotta ensure it's not being held down.
            let pressed = input.state == ElementState::Pressed && !ctx.keyboard.is_pressed(key);

            if settings().key_is_don_or_kat(key) && pressed {
                let time = self.note_time();
                let timing_windows = self.timing_windows();

                // We now have to go through all the notes starting from the next one, and see if
                // any of them react to this keypress. If any of them react, or any of them are too
                // far away to react, then we stop.
                loop {
                    // If there's no next note, we don't need to react.
                    let Some(next_note) = self.notes.get_mut(note_index) else {
                        break;
                    };

                    let reaction = next_note.receive_keypress(key, time, timing_windows);
                    match reaction {
                        // If it's the wrong colour, we'll keep checking to see if there's
                        // a note of the right colour in scope.
                        NoteKeypressReaction::WrongColour => {}

                        NoteKeypressReaction::TooEarly => {
                            // Now we're only looking at notes that are unhittable, so stop here.
                            break;
                        }
                        NoteKeypressReaction::Hit { offset } => {
                            let judgement =
                                NoteJudgement::from_offset(offset, self.timing_windows()).unwrap();
                            self.note_judgement_text.display_judgement(judgement);

                            self.results.judgements.push(Some(judgement));
                            self.results.hit_errors.push(offset);

                            self.next_note_index = note_index + 1;

                            // Ensure you only ever hit one note at a time
                            break;
                        }
                        NoteKeypressReaction::Drumroll { .. } => {
                            self.results.drumrolls += 1;
                            break;
                        }
                        NoteKeypressReaction::BalloonRoll { hits_left, hit_target } => {
                            self.results.drumrolls += 1;
                            self.balloon_display.hit(hits_left, hit_target);

                            if hits_left == 0 {
                                self.next_note_index = note_index + 1;
                            }
                            break;
                        }
                        NoteKeypressReaction::TooLate => {
                            self.skip_next_note();
                        }
                    }

                    note_index += 1;
                }
            }
        }
    }
}
