use std::{rc::Rc, time::Instant};

use kira::{
    manager::AudioManager,
    sound::static_sound::{PlaybackState, StaticSoundData, StaticSoundHandle},
};

use crate::{
    render::{
        self,
        texture::{Sprite, Texture},
    },
    track::{NoteType, Song},
};

use super::{GameState, StateTransition};

const WAIT_SECONDS: f32 = 3.0;
const DRAW_THRESHOLD: f32 = 3.0;
const DISAPPEAR_X: f32 = 550.0;
const DRAW_Y: f32 = 500.0;

// The base velocity is such that at 120 beats per minute, exactly one full measure is shown on the
// screen. This will eventually have to be set based on the current resolution instead of this 
// hardcoded value.
const VELOCITY: f32 = (1920.0 - DISAPPEAR_X) / 2.0;

pub struct TaikoMode {
    song: Rc<Song>,
    difficulty: usize,
    start_time: Option<Instant>,
    song_handle: StaticSoundHandle,
    exit: bool,
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
        big_don_tex: &Rc<Texture>,
        big_kat_tex: &Rc<Texture>,
        renderer: &render::Renderer,
    ) -> Self {
        let mut song_handle = manager.play(song_data).unwrap();
        song_handle.pause(Default::default()).unwrap();

        let sprites = song.difficulties[difficulty]
            .as_ref()
            .unwrap()
            .track
            .notes
            .iter()
            .map(|note| match note.note_type {
                NoteType::Don => Some(Sprite::new(Rc::clone(don_tex), [0.0, 0.0, 0.0], renderer)),
                NoteType::Kat => Some(Sprite::new(Rc::clone(kat_tex), [0.0, 0.0, 0.0], renderer)),
                NoteType::BigDon => Some(Sprite::new(
                    Rc::clone(big_don_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                )),
                NoteType::BigKat => Some(Sprite::new(
                    Rc::clone(big_kat_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                )),
                _ => None,
            })
            .collect();

        Self {
            song,
            difficulty,
            start_time: Some(Instant::now()),
            song_handle,
            exit: false,
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

            let (x, y) = sprite.dimensions();
            let (x_offset, y_offset) = (x as f32 / 2.0, y as f32 / 2.0);

            sprite.set_position(
                [
                    DISAPPEAR_X + VELOCITY * (note.time - current) * note.scroll_speed - x_offset,
                    DRAW_Y - y_offset,
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

            if ui.button("Pause/Play").clicked() && current >= 0.0 {
                println!("{:?}", self.song_handle.state());
                match self.song_handle.state() {
                    PlaybackState::Playing => self.pause_song(),
                    PlaybackState::Paused => self.resume_song(),
                    _ => {}
                }
            }

            self.exit = ui.button("Return").clicked();
        });
    }
}
