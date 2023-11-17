use std::{rc::Rc, time::Instant};

use kira::{
    manager::AudioManager,
    sound::{
        static_sound::{StaticSoundData, StaticSoundHandle},
        PlaybackState,
    },
};
use lyon::{
    geom::point,
    lyon_tessellation::{BuffersBuilder, StrokeOptions},
    path::Path,
};
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, SectionBuilder, VerticalAlign};
use winit::event::{ElementState, WindowEvent};

use silkwood::{
    app::{self, GameState, RenderContext, StateTransition, TextureCache},
    render::{
        self,
        shapes::{LinearGradient, Shape, ShapeBuilder, SolidColour},
        text::Text,
        texture::Sprite,
    },
};

use crate::{
    settings::SETTINGS,
    beatmap_parser::track::{NoteTrack, NoteType, Song},
};

use super::score_screen::ScoreScreen;

mod note;
use note::VisualNote;

// This is a hard-coded value, big enough to make sure that at default scroll speed every note is
// drawn for this long. It will be scaled depending on scroll speed, so every note will be drawn
// for at least as long as it needs to. It's not very elegant but it works.
//
// TODO: No it does not work. For some reason notes that are reeeeeealy slow disappear before
// they're supposed to. See DONKAMA 2000's last note as an example.
const DEFAULT_DRAW_TIME: f32 = 3.0;

// The number of seconds to wait before starting the song
const WAIT_SECONDS: f32 = 3.0;
// The point on the screen where notes should be hit
const NOTE_HIT_X: f32 = 550.0;
// The Y value where notes should be drawn
const NOTE_Y: f32 = 400.0;
const NOTE_FIELD_HEIGHT: f32 = 250.0;

// Colours
const NOTE_FIELD_COLOUR: [f32; 4] = [0.12, 0.12, 0.12, 1.0];
const LEFT_PANEL_COLOUR: [f32; 4] = [0.9, 0.3, 0.2, 1.0];
const NOTE_LINE_COLOUR: [f32; 4] = [0.26, 0.26, 0.26, 1.0];

// The base velocity is such that at 120 beats per minute, exactly one full measure is shown on the
// screen. This will eventually have to be set based on the current resolution instead of this
// hardcoded value.
const VELOCITY: f32 = (1920.0 - NOTE_HIT_X) / 2.0;

// Must again give thanks to OpenTaiko as that's where I found these values.
const EASY_NORMAL_TIMING: [f32; 3] = [0.042, 0.108, 0.125];
const HARD_EXTREME_TIMING: [f32; 3] = [0.025, 0.075, 0.108];

// Indices for the previous timings just to make code look nicer
const GOOD: usize = 0;
const OK: usize = 1;
const BAD: usize = 2;

const JUDGEMENT_TEXT_DISAPPEAR_TIME: f32 = 0.5;

struct UI {
    bg_rect: Shape,
    note_field: Shape,
    note_line: Shape,
    left_panel: Shape,
    title: Text,
    judgement_text: [Text; 3],
}

impl UI {
    fn new(renderer: &mut render::Renderer, song_name: &str) -> anyhow::Result<Self> {
        // The area on which the notes will be travelling
        let note_field = ShapeBuilder::new()
            .filled_rectangle(
                [NOTE_HIT_X - 200.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0],
                [1920.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0],
                SolidColour::new(NOTE_FIELD_COLOUR),
            )?
            .build(&renderer.device);

        let bg_rect = ShapeBuilder::new()
            // a dark grey gradient above the note field
            .filled_rectangle(
                [0., 0.],
                [1920., NOTE_Y - NOTE_FIELD_HEIGHT / 2.0],
                LinearGradient::new(
                    [0.15, 0.15, 0.15, 0.9],
                    [0.0, 0.0, 0.0, 1.0],
                    [0.0, 0.0],
                    [0.0, 1.0],
                )
                .unwrap(),
            )?
            // A translucent black area below the note field
            .filled_rectangle(
                [0., NOTE_Y],
                [1920., 1080.],
                SolidColour::new([0.0, 0.0, 0.0, 0.8]),
            )?
            .build(&renderer.device);

        // The marquee that shows where notes should be hit
        let note_line = ShapeBuilder::new()
            .stroke_shape(|tess, out| {
                let mut path = Path::builder();
                path.begin(point(NOTE_HIT_X, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0));
                path.line_to(point(NOTE_HIT_X, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0));
                path.end(false);

                let options = StrokeOptions::DEFAULT.with_line_width(4.0);
                let mut builder = BuffersBuilder::new(out, SolidColour::new(NOTE_LINE_COLOUR));

                // A line that shows exactly where notes should be hit
                tess.tessellate_path(&path.build(), &options, &mut builder)?;

                // The outline of a small note
                tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 50.0, &options, &mut builder)?;

                // The outline of a large note
                tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 75.0, &options, &mut builder)?;

                Ok(())
            })?
            .build(&renderer.device);

        // A panel over the left hand side of the note field where notes will disappear under.
        // Also will show combo and a visual representation of the drum input
        let left_panel = ShapeBuilder::new()
            .filled_rectangle(
                [0.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0],
                [NOTE_HIT_X - 203.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0],
                SolidColour::new(LEFT_PANEL_COLOUR),
            )?
            .filled_rectangle(
                [NOTE_HIT_X - 203.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0],
                [NOTE_HIT_X - 200.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0],
                SolidColour::new([0.0, 0.0, 0.0, 1.0]),
            )?
            .build(&renderer.device);

        let title = SectionBuilder::default()
            .with_screen_position((1820.0, 40.0))
            .with_layout(Layout::default().h_align(HorizontalAlign::Right))
            .with_text(vec![wgpu_text::glyph_brush::Text::new(song_name)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(80.0)]);

        let title = Text::new_outlined(renderer, &title).unwrap();

        let mut build_judgement_text = |text, colour| {
            let section = SectionBuilder::default()
                .with_screen_position((NOTE_HIT_X, NOTE_Y - 75.0))
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

        let judgement_text = [
            build_judgement_text("Good", [1.0, 198.0 / 255.0, 41.0 / 255.0, 1.0]),
            build_judgement_text("Ok", [1.0; 4]),
            build_judgement_text("Bad", [46.0 / 255.0, 103.0 / 255.0, 209.0 / 255.0, 1.0]),
        ];

        Ok(Self {
            bg_rect,
            note_field,
            note_line,
            left_panel,
            title,
            judgement_text,
        })
    }
}

// Contains only the necessary info for the current song in a convenient place so we don't have to
// go digging around through shared pointers to find the important info like notes
struct CurrentSong {
    title: String,
    difficulty_level: usize,
    track: NoteTrack,
}

impl CurrentSong {
    fn from_song(song: &Song, difficulty: usize) -> Option<Self> {
        Some(Self {
            title: song.title.clone(),
            difficulty_level: difficulty,
            track: song.difficulties.get(difficulty)?.as_ref()?.track.clone(),
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum HitState {
    Bad,
    Ok,
    Good,
}

pub struct TaikoMode {
    song: CurrentSong,
    audio_handle: StaticSoundHandle,
    start_time: Option<Instant>,
    exit: bool,
    visual_notes: Vec<Option<VisualNote>>,
    visual_barlines: Vec<Shape>,
    elapsed: f32,
    paused: bool,
    started: bool,

    // every time i decide on an integer size i have to ask myself a philosophical question
    // is it unreasonable to assume no rhythm gamer will achieve a 4,294,967,296 combo?
    combo: u32,
    max_combo: u32,
    next_note: usize,

    note_offsets: Vec<f32>,

    hits: Vec<Option<HitState>>,
    last_hit: Option<(HitState, f32)>,
    ui: UI,
    bg_sprite: Rc<Sprite>,
}

impl TaikoMode {
    pub fn new(
        song: &Song,
        difficulty: usize,
        song_data: StaticSoundData,
        manager: &mut AudioManager,
        textures: &mut TextureCache,
        renderer: &mut render::Renderer,
        bg_sprite: &Rc<Sprite>,
    ) -> Option<Self> {
        let mut song_handle = manager.play(song_data).unwrap();
        song_handle.pause(Default::default()).unwrap();

        let device = &renderer.device;
        let queue = &renderer.queue;

        let song = CurrentSong::from_song(song, difficulty)?;

        let visual_notes = song
            .track
            .notes
            .iter()
            .map(|note| {
                VisualNote::new(
                    device,
                    queue,
                    &note.note_type,
                    VELOCITY * note.scroll_speed,
                    textures,
                )
            })
            .collect::<Vec<_>>();

        let visual_barlines = song
            .track
            .barlines
            .iter()
            .map(|_| {
                ShapeBuilder::new()
                    .filled_rectangle(
                        [-1.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0],
                        [1.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0],
                        SolidColour::new([1.0, 1.0, 1.0, 0.5]),
                    )
                    .unwrap()
                    .build(device)
            })
            .collect::<Vec<_>>();

        let notes = visual_notes.len();

        let ui = UI::new(renderer, &song.title).unwrap();

        Some(Self {
            song,
            start_time: Some(Instant::now()),
            audio_handle: song_handle,
            exit: false,
            visual_notes,
            visual_barlines,
            elapsed: 0.0,
            paused: false,
            started: false,
            combo: 0,
            max_combo: 0,
            next_note: 0,
            note_offsets: vec![],
            hits: vec![None; notes],
            last_hit: None,
            ui,
            bg_sprite: Rc::clone(bg_sprite),
        })
    }

    fn current_time(&self) -> f32 {
        self.total_elapsed_time() - WAIT_SECONDS
    }

    fn total_elapsed_time(&self) -> f32 {
        self.elapsed
            + self
                .start_time
                .map(|time| time.elapsed().as_secs_f32())
                .unwrap_or_default()
    }

    fn pause_song(&mut self) {
        self.elapsed = self.total_elapsed_time();
        self.start_time = None;
        self.paused = true;
        self.audio_handle.pause(Default::default()).unwrap();
    }

    fn resume_song(&mut self) {
        self.start_time = Some(Instant::now());
        self.paused = false;
        self.audio_handle.resume(Default::default()).unwrap();
    }
}

impl GameState for TaikoMode {
    fn update(&mut self, ctx: &mut app::Context, _dt: f32) -> StateTransition {
        if !self.paused {
            let current = self.current_time();

            // Start the song if we reach 0 seconds
            if current >= 0.0 && !self.started {
                self.audio_handle.resume(Default::default()).unwrap();
                self.started = true;
            }

            let note_current = current - SETTINGS.read().unwrap().game.global_note_offset / 1000.0;

            let timings = if self.song.difficulty_level <= 1 {
                EASY_NORMAL_TIMING
            } else {
                HARD_EXTREME_TIMING
            };

            // Go through all the notes that weren't hit and remove them
            // (this will do more eventually, currently it just resets combo)
            for note in &self.song.track.notes[self.next_note..] {
                if note.time >= note_current - timings[BAD] {
                    break;
                }

                if self.hits[self.next_note].is_none() && !note.note_type.is_roll() {
                    self.combo = 0;
                }

                self.next_note += 1;
            }
        }

        if self.audio_handle.state() == PlaybackState::Stopped {
            let goods = self
                .hits
                .iter()
                .filter(|state| **state == Some(HitState::Good))
                .count() as u32;
            let okays = self
                .hits
                .iter()
                .filter(|state| **state == Some(HitState::Ok))
                .count() as u32;
            let bads = self
                .hits
                .iter()
                .filter(|state| **state == Some(HitState::Bad))
                .count() as u32;
            let misses = self
                .hits
                .iter()
                .enumerate()
                .filter(|(i, state)| {
                    !self.song.track.notes[*i].note_type.is_roll() && state.is_none()
                })
                .count() as u32;

            StateTransition::Swap(Box::new(ScoreScreen::new(
                ctx,
                goods,
                okays,
                bads + misses,
                0,
                self.max_combo,
                &self.song.title,
            )))
        } else if self.exit {
            self.audio_handle.stop(Default::default()).unwrap();
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }

    fn render<'pass>(&'pass mut self, ctx: &mut RenderContext<'_, 'pass>) {
        let current =
            self.current_time() - SETTINGS.read().unwrap().game.global_note_offset / 1000.0;
        let notes = &self.song.track.notes;

        let draw_notes = self
            .visual_notes
            .iter_mut()
            .enumerate()
            .filter_map(|(i, sprite)| {
                let note = notes[i];

                if ((current - 1.0)..current + DEFAULT_DRAW_TIME / note.scroll_speed)
                    .contains(&(note.time))
                    && self.hits[i].is_none()
                {
                    sprite.as_mut().map(|s| (s, i))
                } else {
                    None
                }
            });

        let draw_barlines =
            self.visual_barlines
                .iter_mut()
                .enumerate()
                .filter_map(|(i, visual_barline)| {
                    let barline = self.song.track.barlines[i];

                    if ((current - 1.0)..current + DEFAULT_DRAW_TIME / barline.scroll_speed)
                        .contains(&(barline.time))
                    {
                        Some((visual_barline, i))
                    } else {
                        None
                    }
                });

        ctx.render(self.bg_sprite.as_ref());
        ctx.render(&self.ui.bg_rect);
        ctx.render(&self.ui.note_field);
        ctx.render(&self.ui.note_line);

        for (v_barline, barline_index) in draw_barlines {
            let barline = &self.song.track.barlines[barline_index];

            v_barline.set_position(
                [
                    NOTE_HIT_X + VELOCITY * (barline.time - current) * barline.scroll_speed,
                    0.0,
                    0.0,
                ],
                ctx.render_pass.queue,
            );

            ctx.render(v_barline);
        }

        for (v_note, note_index) in draw_notes.rev() {
            let note = &notes[note_index];
            v_note.set_position(
                [
                    NOTE_HIT_X + VELOCITY * (note.time - current) * note.scroll_speed,
                    NOTE_Y,
                    note.time,
                ],
                ctx.render_pass.queue,
            );

            ctx.render(v_note)
        }

        if let Some((result, time)) = self.last_hit {
            if current - time <= JUDGEMENT_TEXT_DISAPPEAR_TIME {
                let i = match result {
                    HitState::Good => GOOD,
                    HitState::Ok => OK,
                    HitState::Bad => BAD,
                };

                let progress = ((current - time) / JUDGEMENT_TEXT_DISAPPEAR_TIME).powf(0.1);

                self.ui.judgement_text[i]
                    .sprite
                    .set_position([0.0, -10.0 * progress + 5.0, 0.0], ctx.render_pass.queue);

                ctx.render(&self.ui.judgement_text[i]);
            }
        }

        ctx.render(&self.ui.left_panel);
        ctx.render(&self.ui.title);
    }

    fn debug_ui(&mut self, ctx: egui::Context, _audio: &mut AudioManager) {
        egui::Window::new("taiko mode debug menu").show(&ctx, |ui| {
            let current = self.current_time();
            ui.label(format!("song time: {current}"));

            if ui.button("Pause/Play").clicked() && current >= 0.0 {
                match self.audio_handle.state() {
                    PlaybackState::Playing => self.pause_song(),
                    PlaybackState::Paused => self.resume_song(),
                    _ => {}
                }
            }

            if let Some((result, time)) = self.last_hit {
                if current - time <= JUDGEMENT_TEXT_DISAPPEAR_TIME {
                    ui.label(format!("{result:?}"));
                }
            }

            ui.label(format!("combo: {}", self.combo));

            if !self.note_offsets.is_empty() {
                let average =
                    self.note_offsets.iter().sum::<f32>() / self.note_offsets.len() as f32;
                ui.label(format!("average offset: {}", average));
            }

            self.exit = ui.button("Return").clicked();
        });
    }

    fn handle_event(&mut self, ctx: &mut app::Context, event: &WindowEvent<'_>) {
        let mut current = self.current_time();
        let settings = SETTINGS.read().unwrap();
        let offset = settings.game.global_note_offset / 1000.0;
        current = current - offset;

        if let &WindowEvent::KeyboardInput {
            input,
            is_synthetic: false,
            ..
        } = event
        {
            if let Some(code) = input.virtual_keycode {
                let pressed =
                    !ctx.keyboard.is_pressed(code) && input.state == ElementState::Pressed;
                if pressed {
                    let timings = if self.song.difficulty_level <= 1 {
                        EASY_NORMAL_TIMING
                    } else {
                        HARD_EXTREME_TIMING
                    };

                    let don_keys = [
                        settings.game.key_mappings.left_don,
                        settings.game.key_mappings.right_don,
                    ];
                    let kat_keys = [
                        settings.game.key_mappings.left_ka,
                        settings.game.key_mappings.right_ka,
                    ];

                    if don_keys.contains(&code) {
                        let next_don =
                            self.song.track.notes.iter().enumerate().find(|(i, note)| {
                                let note_time_difference = (note.time - current).abs();

                                note_time_difference <= timings[BAD]
                                    && matches!(note.note_type, NoteType::Don | NoteType::BigDon)
                                    && self.hits[*i].is_none()
                            });

                        if let Some((i, note)) = next_don {
                            let note_time_difference = (note.time - current).abs();
                            self.note_offsets.push(current - note.time);

                            let result = if note_time_difference <= timings[GOOD] {
                                Some(HitState::Good)
                            } else if note_time_difference <= timings[OK] {
                                Some(HitState::Ok)
                            } else {
                                Some(HitState::Bad)
                            };

                            self.last_hit = result.map(|state| (state, current));
                            self.hits[i] = result;

                            if result == Some(HitState::Bad) {
                                self.combo = 0;
                            } else {
                                self.combo += 1;

                                if self.combo > self.max_combo {
                                    self.max_combo = self.combo;
                                }
                            }
                        }
                    }

                    if kat_keys.contains(&code) {
                        let next_kat =
                            self.song.track.notes.iter().enumerate().find(|(i, note)| {
                                let note_time_difference = (note.time - current).abs();

                                note_time_difference <= timings[BAD]
                                    && matches!(note.note_type, NoteType::Kat | NoteType::BigKat)
                                    && self.hits[*i].is_none()
                            });

                        if let Some((i, note)) = next_kat {
                            let note_time_difference = (note.time - current).abs();
                            self.note_offsets.push(current - note.time);

                            let result = if note_time_difference <= timings[GOOD] {
                                Some(HitState::Good)
                            } else if note_time_difference <= timings[OK] {
                                Some(HitState::Ok)
                            } else {
                                Some(HitState::Bad)
                            };

                            self.last_hit = result.map(|state| (state, current));
                            self.hits[i] = result;

                            if result == Some(HitState::Bad) {
                                self.combo = 0;
                            } else {
                                self.combo += 1;

                                if self.combo > self.max_combo {
                                    self.max_combo = self.combo;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
