use std::{collections::HashMap, rc::{Rc, Weak}};

use kira::manager::{backend::DefaultBackend, AudioManager};

mod song_select;
use song_select::SongSelect;

use crate::render::{self, texture::{Texture, Sprite}};

const FPS_POLL_TIME: f32 = 0.5;

pub enum StateTransition {
    Continue,
    Push(Box<dyn GameState>),
    Swap(Box<dyn GameState>),
    Pop,
}

pub trait GameState {
    // TODO: Make a context struct instead of passing in the raw audio manager
    fn update(&mut self, _delta: f32, _audio: &mut AudioManager, _renderer: &render::Renderer) -> StateTransition {
        StateTransition::Continue
    }
    fn debug_ui(&mut self, _ctx: egui::Context, _audio: &mut AudioManager) {}
    fn render<'a>(
        &'a mut self,
        _renderer: &'a render::Renderer,
        _render_pass: &mut wgpu::RenderPass<'a>,
    ) {
    }
}

pub struct App {
    audio_manager: AudioManager,

    // TODO: Write a resources manager struct for this kind of thing
    textures: HashMap<&'static str, Weak<Texture>>,

    state: Vec<Box<dyn GameState>>,

    fps_counter: f32,
    frames_counted: u32,
    fps: f32,
}

impl App {
    pub fn new(renderer: &render::Renderer) -> anyhow::Result<Self> {
        let audio_manager = AudioManager::<DefaultBackend>::new(Default::default())?;
        let bg_filename = "assets/song_select_bg.jpg";
        let bg_texture = Rc::new(Texture::from_file(bg_filename, renderer)?);
        let bg_texture_weak = Rc::downgrade(&bg_texture);

        let state = Box::new(SongSelect::new(Sprite::new(bg_texture, [0.0, 0.0, 0.0], renderer))?);

        let mut textures = HashMap::new();
        textures.insert(bg_filename, bg_texture_weak);

        Ok(App {
            audio_manager,
            state: vec![state],
            textures, 
            fps_counter: 0.0,
            frames_counted: 0,
            fps: 0.0,
        })
    }

    pub fn update(&mut self, delta: f32, renderer: &render::Renderer) {
        self.fps_counter += delta;
        self.frames_counted += 1;

        if self.fps_counter >= FPS_POLL_TIME {
            self.fps = self.frames_counted as f32 / self.fps_counter;
            self.fps_counter = 0.0;
            self.frames_counted = 0;
        }

        match self
            .state
            .last_mut()
            .unwrap()
            .update(delta, &mut self.audio_manager, renderer)
        {
            StateTransition::Push(state) => self.state.push(state),
            StateTransition::Pop => {
                self.state
                    .pop()
                    .expect("found no previous state to return to!");
            }
            StateTransition::Swap(state) => *self.state.last_mut().unwrap() = state,
            StateTransition::Continue => {}
        }
    }

    pub fn debug_ui(&mut self, ctx: egui::Context) {
        self.state
            .last_mut()
            .unwrap()
            .debug_ui(ctx.clone(), &mut self.audio_manager);

        egui::Area::new("fps counter")
            .fixed_pos(egui::pos2(1800.0, 0.0))
            .show(&ctx, |ui| {
                ui.label(egui::RichText::new(format!("fps: {:.2}", self.fps)).color(egui::Color32::from_rgb(255, 0, 255)).size(20.0));
            });
    }

    pub fn render<'a>(
        &'a mut self,
        renderer: &'a render::Renderer,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        self.state.last_mut().unwrap().render(renderer, render_pass)
    }
}
