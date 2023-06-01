// Note to self and anyone reading this
// a lot of the timing logic in this code will probably be quite off because this is just a demo
// fix that

use std::{rc::Rc, time::Instant};

use kira::{
    manager::AudioManager,
    sound::static_sound::{StaticSoundData, StaticSoundHandle, PlaybackState},
};

use crate::{
    render::{
        self,
        texture::{Sprite, Texture},
    },
    track::{Note, NoteType, Song},
};

use super::{GameState, StateTransition};

const WAIT_SECONDS: f32 = 3.0;
const DRAW_THRESHOLD: f32 = 3.0;
const DISAPPEAR_POS: f32 = 400.0;
const VELOCITY: f32 = 600.0;
const DRAW_Y: f32 = 500.0;

pub struct TaikoMode {
    song: Rc<Song>,
    difficulty: usize,
    start_time: Option<Instant>,
    song_handle: StaticSoundHandle,
    exit: bool,
    next_note: Option<Note>,
    sprites: Vec<Option<Sprite>>,
    elapsed: f32,
    paused: bool,
    started: bool,
}

impl TaikoMode {
    pub fn new(
        song: Rc<Song>,
        difficulty: usize,
        song_data: StaticSoundData,
        manager: &mut AudioManager,
        don_tex: &Rc<Texture>,
        kat_tex: &Rc<Texture>,
        renderer: &render::Renderer,
    ) -> Self {
        let mut song_handle = manager.play(song_data.clone()).unwrap();
        song_handle.pause(Default::default()).unwrap();

        let sprites = song.difficulties[difficulty]
            .as_ref()
            .unwrap()
            .track
            .notes
            .iter()
            .map(|note| match note.note_type {
                NoteType::Don | NoteType::BigDon => {
                    Some(Sprite::new(Rc::clone(don_tex), [0.0, 0.0, 0.0], renderer))
                }
                NoteType::Kat | NoteType::BigKat => {
                    Some(Sprite::new(Rc::clone(kat_tex), [0.0, 0.0, 0.0], renderer))
                }
                _ => None,
            })
            .collect();

        Self {
            song,
            difficulty,
            start_time: Some(Instant::now()),
            song_handle,
            exit: false,
            next_note: None,
            sprites,
            elapsed: 0.0,
            paused: false,
            started: false,
        }
    }

    fn current_time(&self) -> f32 {
        self.total_elapsed_time() - WAIT_SECONDS
    }

    fn total_elapsed_time(&self) -> f32 {
        self.elapsed + self.start_time.map(|time| time.elapsed().as_secs_f32()).unwrap_or_default()
    }

    fn pause_song(&mut self) {
        self.elapsed = self.total_elapsed_time();
        self.start_time = None;
        self.paused = true;
        self.song_handle.pause(Default::default()).unwrap();
    }

    fn resume_song(&mut self) {
        self.start_time = Some(Instant::now());
        self.paused = false;
        self.song_handle.resume(Default::default()).unwrap();
    }
}

impl GameState for TaikoMode {
    fn update(
        &mut self,
        _delta: f32,
        _audio: &mut AudioManager,
        _renderer: &crate::render::Renderer,
    ) -> StateTransition {
        if !self.paused {
            let current = self.current_time();

            if current >= 0.0 && !self.started {
                self.song_handle.resume(Default::default()).unwrap();
                self.started = true;
            }

            self.next_note = self.song.difficulties[self.difficulty]
                .as_ref()
                .unwrap()
                .track
                .notes
                .iter()
                .find(|note| note.time >= current)
                .cloned();
        }

        if self.exit {
            self.song_handle.stop(Default::default()).unwrap();
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }

    fn render<'a>(
        &'a mut self,
        renderer: &'a render::Renderer,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let current = self.current_time();
        let notes = &self.song.difficulties[self.difficulty]
            .as_ref()
            .unwrap()
            .track
            .notes;

        let draw_sprites = self
            .sprites
            .iter_mut()
            .enumerate()
            .filter_map(|(i, sprite)| {
                let note = notes[i];

                if (current..current + DRAW_THRESHOLD / note.scroll_speed).contains(&(note.time)) {
                    sprite.as_mut().map(|s| (s, i))
                } else {
                    None
                }
            });

        for (sprite, note_index) in draw_sprites {
            let note = &notes[note_index];

            sprite.set_position(
                [
                    DISAPPEAR_POS + VELOCITY * (note.time - current) * note.scroll_speed,
                    DRAW_Y,
                    note.time,
                ],
                renderer,
            );
            sprite.render(renderer, render_pass);
        }
    }

    fn debug_ui(&mut self, ctx: egui::Context, _audio: &mut AudioManager) {
        egui::Window::new("taiko mode debug menu").show(&ctx, |ui| {
            let current = self.current_time();
            ui.label(format!("song time: {}", current));

            let mut note_str = "Note: ".to_string();

            if let Some(note) = self.next_note.as_ref() {
                if (note.time - current).abs() < 0.1 {
                    note_str.push_str(&format!("{:?}", note.note_type));
                }
            }

            ui.label(note_str);

            if ui.button("Pause/Play").clicked() && current >= 0.0 {
                println!("{:?}", self.song_handle.state());
                match self.song_handle.state() {
                    PlaybackState::Playing => self.pause_song(),
                    PlaybackState::Paused => self.resume_song(),
                    _ => {},
                }
            }

            self.exit = ui.button("Return").clicked();
        });
    }
}
