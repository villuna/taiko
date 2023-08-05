// TODO: Refactor this. Create a new temporary song struct to be used just by this module.
use std::{rc::Rc, time::Instant};

use kira::{
    manager::AudioManager,
    sound::static_sound::{PlaybackState, StaticSoundData, StaticSoundHandle},
};
use lyon::{
    geom::{point, Box2D},
    lyon_tessellation::{BuffersBuilder, FillOptions, StrokeOptions},
    path::Path,
};
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, SectionBuilder};
use winit::event::{ElementState, VirtualKeyCode, WindowEvent};

use crate::{
    render::{
        self,
        primitives::{Primitive, SolidColour, LinearGradient},
        texture::{Sprite, Texture},
        text::Text,
    },
    track::{NoteType, Song},
};

use super::{GameState, StateTransition};

// This is a hard-coded value, big enough to make sure that at default scroll speed every note is
// drawn for this long. It will be scaled depending on scroll speed, so every note will be drawn
// for at least as long as it needs to. It's not very elegant but it works.
const DEFAULT_DRAW_TIME: f32 = 3.0;
// The number of seconds to wait before starting the song
const WAIT_SECONDS: f32 = 3.0;
// The point on the screen where notes should be hit
const NOTE_HIT_X: f32 = 550.0;
// The Y value where notes should be drawn
const NOTE_Y: f32 = 400.0;
const NOTE_FIELD_HEIGHT: f32 = 250.0;

// The base velocity is such that at 120 beats per minute, exactly one full measure is shown on the
// screen. This will eventually have to be set based on the current resolution instead of this
// hardcoded value.
const VELOCITY: f32 = (1920.0 - NOTE_HIT_X) / 2.0;
const DON_KEYS: &[VirtualKeyCode] = &[VirtualKeyCode::S, VirtualKeyCode::Numpad4];
const KAT_KEYS: &[VirtualKeyCode] = &[VirtualKeyCode::A, VirtualKeyCode::Numpad5];
const OK_WINDOW: f32 = 0.1;

struct UI {
    bg_rect: Primitive,
    note_field: Primitive,
    note_line: Primitive,
    left_panel: Primitive,
    title: Text,
}

impl UI {
    fn new(renderer: &mut render::Renderer, song_name: &str) -> anyhow::Result<Self> {
        let bg_rect = Primitive::filled_shape(&renderer.device, |tess, out| {
            tess.tessellate_rectangle(
                &Box2D::new(point(0.0, 0.0), point(1920.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0)),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(
                    out,
                    LinearGradient::new(
                        [0.05, 0.05, 0.05, 0.9],
                        [0.0, 0.0, 0.0, 1.0],
                        [0.0, 0.0],
                        [0.0, 1.0],
                    ).unwrap(),
                ),
            )?;

            tess.tessellate_rectangle(
                &Box2D::new(point(0.0, NOTE_Y), point(1920.0, 1080.0)),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(
                    out,
                    SolidColour::new([0.0, 0.0, 0.0, 0.8])
                ),
            )?;

            Ok(())
        })?;

        let note_field = Primitive::filled_shape(&renderer.device, |tess, out| {
            tess.tessellate_rectangle(
                &Box2D::new(
                    point(NOTE_HIT_X - 200.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0),
                    point(1920.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0),
                ),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(
                    out,
                    SolidColour::new([0.01, 0.01, 0.01, 1.0])
                ),
            )?;

            Ok(())
        })?;

        let note_line = Primitive::stroke_shape(&renderer.device, |tess, out| {
            let mut path = Path::builder();
            path.begin(point(NOTE_HIT_X, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0));
            path.line_to(point(NOTE_HIT_X, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0));
            path.end(false);

            let options = StrokeOptions::DEFAULT.with_line_width(4.0);
            let mut builder = BuffersBuilder::new(
                out,
                SolidColour::new([0.05, 0.05, 0.05, 1.0])
            );

            // A line that shows exactly where notes should be hit
            tess.tessellate_path(&path.build(), &options, &mut builder)?;

            // The outline of a small note
            tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 50.0, &options, &mut builder)?;

            // The outline of a large note
            tess.tessellate_circle(point(NOTE_HIT_X, NOTE_Y), 75.0, &options, &mut builder)?;

            Ok(())
        })?;

        let left_panel = Primitive::filled_shape(&renderer.device, |tess, out| {
            tess.tessellate_rectangle(
                &Box2D::new(
                    point(0.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0),
                    point(NOTE_HIT_X - 203.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0),
                ),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(
                    out,
                    SolidColour::new([0.8, 0.07, 0.03, 1.0])
                ),
            )?;

            tess.tessellate_rectangle(
                &Box2D::new(
                    point(NOTE_HIT_X - 203.0, NOTE_Y - NOTE_FIELD_HEIGHT / 2.0),
                    point(NOTE_HIT_X - 200.0, NOTE_Y + NOTE_FIELD_HEIGHT / 2.0),
                ),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(
                    out,
                    SolidColour::new([0.0, 0.0, 0.0, 1.0])
                ),
            )?;

            Ok(())
        })?;

        let title = SectionBuilder::default()
            .with_screen_position((1820.0, 40.0))
            .with_layout(Layout::default().h_align(HorizontalAlign::Right))
            .with_text(vec![wgpu_text::glyph_brush::Text::new(
                song_name,
            )
            .with_color([1.0, 1.0, 1.0, 1.0])
            .with_scale(80.0)]);

        let title = Text::new_outlined(renderer, &title).unwrap();

        Ok(Self {
            bg_rect,
            note_field,
            note_line,
            left_panel,
            title,
        })
    }
}

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
    hits: Vec<bool>,
    last_hit: Option<NoteType>,
    ui: UI,
    bg_sprite: Rc<Sprite>,
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
        roll_start_tex: &Rc<Texture>,
        big_roll_start_tex: &Rc<Texture>,
        renderer: &mut render::Renderer,
        bg_sprite: &Rc<Sprite>,
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
                NoteType::Don => Some(Sprite::new(
                    Rc::clone(don_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                    true,
                )),
                NoteType::Kat => Some(Sprite::new(
                    Rc::clone(kat_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                    true,
                )),
                NoteType::BigDon => Some(Sprite::new(
                    Rc::clone(big_don_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                    true,
                )),
                NoteType::BigKat => Some(Sprite::new(
                    Rc::clone(big_kat_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                    true,
                )),
                NoteType::Roll(_) => Some(Sprite::new(
                    Rc::clone(roll_start_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                    true,
                )),
                NoteType::BigRoll(_) => Some(Sprite::new(
                    Rc::clone(big_roll_start_tex),
                    [0.0, 0.0, 0.0],
                    renderer,
                    true,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        let notes = sprites.len();

        let ui = UI::new(renderer, &song.title).unwrap();

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
            hits: vec![false; notes],
            last_hit: None,
            ui,
            bg_sprite: Rc::clone(bg_sprite),
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
        _ctx: &mut super::Context,
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

    fn render<'a>(&'a mut self, ctx: &mut render::RenderContext<'a>) {
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

                if ((current - 1.0)..current + DEFAULT_DRAW_TIME / note.scroll_speed)
                    .contains(&(note.time))
                    && !self.hits[i]
                {
                    sprite.as_mut().map(|s| (s, i))
                } else {
                    None
                }
            });

        ctx.render(self.bg_sprite.as_ref());
        ctx.render(&self.ui.bg_rect);
        ctx.render(&self.ui.note_field);
        ctx.render(&self.ui.note_line);

        for (sprite, note_index) in draw_sprites.rev() {
            let note = &notes[note_index];

            let (x, y) = sprite.dimensions();
            let (x_offset, y_offset) = (x as f32 / 2.0, y as f32 / 2.0);

            sprite.set_position(
                [
                    NOTE_HIT_X + VELOCITY * (note.time - current) * note.scroll_speed - x_offset,
                    NOTE_Y - y_offset,
                    note.time,
                ],
                ctx.queue,
            );
            ctx.render(sprite)
        }

        ctx.render(&self.ui.left_panel);
        ctx.render(&self.ui.title);
    }

    fn debug_ui(&mut self, ctx: egui::Context, _audio: &mut AudioManager) {
        egui::Window::new("taiko mode debug menu").show(&ctx, |ui| {
            let current = self.current_time();
            ui.label(format!("song time: {current}"));
            ui.label(format!("last hit: {:?}", self.last_hit));

            if ui.button("Pause/Play").clicked() && current >= 0.0 {
                match self.song_handle.state() {
                    PlaybackState::Playing => self.pause_song(),
                    PlaybackState::Paused => self.resume_song(),
                    _ => {}
                }
            }

            self.exit = ui.button("Return").clicked();
        });
    }

    fn handle_event(&mut self, event: &WindowEvent<'_>, keyboard: &super::KeyboardState) {
        if let &WindowEvent::KeyboardInput {
            input,
            is_synthetic: false,
            ..
        } = event
        {
            if let Some(code) = input.virtual_keycode {
                let pressed = !keyboard.is_pressed(code) && input.state == ElementState::Pressed;

                if pressed {
                    // Don
                    let current = self.current_time();

                    if DON_KEYS.contains(&code) {
                        let next_don = self.song.difficulties[self.difficulty]
                            .as_ref()
                            .unwrap()
                            .track
                            .notes
                            .iter()
                            .enumerate()
                            .find(|(i, note)| {
                                (note.time - current).abs() <= OK_WINDOW
                                    && matches!(note.note_type, NoteType::Don | NoteType::BigDon)
                                    && !self.hits[*i]
                            });

                        if let Some((i, note)) = next_don {
                            self.hits[i] = true;
                            self.last_hit = Some(note.note_type)
                        }
                    }

                    if KAT_KEYS.contains(&code) {
                        let next_don = self.song.difficulties[self.difficulty]
                            .as_ref()
                            .unwrap()
                            .track
                            .notes
                            .iter()
                            .enumerate()
                            .find(|(i, note)| {
                                (note.time - current).abs() <= OK_WINDOW
                                    && matches!(note.note_type, NoteType::Kat | NoteType::BigKat)
                                    && !self.hits[*i]
                            });

                        if let Some((i, note)) = next_don {
                            self.hits[i] = true;
                            self.last_hit = Some(note.note_type)
                        }
                    }
                }
            }
        }
    }
}
