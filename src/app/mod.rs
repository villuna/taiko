use kira::manager::{backend::DefaultBackend, AudioManager};

mod song_select;
use song_select::SongSelect;

use crate::render;

pub enum StateTransition {
    Continue,
    Push(Box<dyn GameState>),
    Swap(Box<dyn GameState>),
    Pop,
}

pub trait GameState {
    // TODO: Make a context struct instead of passing in the raw audio manager
    fn update(&mut self, _delta: f32, _audio: &mut AudioManager) -> StateTransition {
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
    state: Vec<Box<dyn GameState>>,
}

impl App {
    pub fn new(renderer: &render::Renderer) -> anyhow::Result<Self> {
        let audio_manager = AudioManager::<DefaultBackend>::new(Default::default())?;
        let state = Box::new(SongSelect::new(renderer)?);

        Ok(App {
            audio_manager,
            state: vec![state],
        })
    }

    pub fn update(&mut self, delta: f32) {
        match self
            .state
            .last_mut()
            .unwrap()
            .update(delta, &mut self.audio_manager)
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
            .debug_ui(ctx, &mut self.audio_manager);
    }

    pub fn render<'a>(
        &'a mut self,
        renderer: &'a render::Renderer,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        self.state.last_mut().unwrap().render(renderer, render_pass)
    }
}
