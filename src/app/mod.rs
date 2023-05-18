use kira::manager::{backend::DefaultBackend, AudioManager};

mod song_select;
use song_select::SongSelect;

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
}

pub struct App {
    audio_manager: AudioManager,
    state: Vec<Box<dyn GameState>>,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let audio_manager = AudioManager::<DefaultBackend>::new(Default::default())?;
        let state = Box::new(SongSelect::new()?);

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
            },
            StateTransition::Swap(state) => {
                *self.state.last_mut().unwrap() = state
            }
            StateTransition::Continue => {}
        }
    }

    pub fn debug_ui(&mut self, ctx: egui::Context) {
        self.state
            .last_mut()
            .unwrap()
            .debug_ui(ctx, &mut self.audio_manager);
    }
}
