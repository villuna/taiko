use std::rc::Rc;

use kira::manager::{backend::DefaultBackend, AudioManager};
use std::collections::HashMap;

mod credits;
mod song_select;
mod taiko_mode;

use song_select::SongSelect;
use winit::{
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

use crate::render::{
    self,
    texture::{Sprite, Texture},
};

const FPS_POLL_TIME: f32 = 0.5;

pub enum StateTransition {
    Continue,
    Push(Box<dyn GameState>),
    Swap(Box<dyn GameState>),
    Pop,
    Exit,
}

pub trait GameState {
    // TODO: Make a context struct instead of passing in the raw audio manager
    fn update(
        &mut self,
        _delta: f32,
        _audio: &mut AudioManager,
        _renderer: &render::Renderer,
    ) -> StateTransition {
        StateTransition::Continue
    }

    fn debug_ui(&mut self, _ctx: egui::Context, _audio: &mut AudioManager) {}

    fn render<'a>(&'a mut self, _ctx: &mut render::RenderContext<'a>) {}

    fn handle_event(&mut self, _event: &WindowEvent<'_>, _keyboard: &KeyboardState) {}
}

/// A struct that keeps track of the state of the keyboard at each frame.
///
/// Each keycode is mapped to a tuple containing two booleans; the first indicates whether the key
/// was pressed last frame, the second indicates whether the key is pressed this frame.
pub struct KeyboardState(HashMap<VirtualKeyCode, (bool, bool)>);

impl KeyboardState {
    fn handle_input(&mut self, event: &KeyboardInput) {
        if let Some(code) = event.virtual_keycode {
            let pressed = event.state == ElementState::Pressed;

            self.0.entry(code).or_insert((false, false)).1 = pressed;
        }
    }

    /// Returns whether or not the given key is pressed this frame.
    pub fn is_pressed(&self, key: VirtualKeyCode) -> bool {
        self.0
            .get(&key)
            .map(|(_, pressed)| *pressed)
            .unwrap_or(false)
    }

    /// Returns whether or not the given key was just pressed this frame (i.e: pressed this frame
    /// but not last frame)
    pub fn is_just_pressed(&self, key: VirtualKeyCode) -> bool {
        self.0
            .get(&key)
            .map(|(last_frame, this_frame)| !(*last_frame) && *this_frame)
            .unwrap_or(false)
    }

    /// Returns whether or not the given key was just released this frame (i.e: released this frame
    /// but not last frame)
    pub fn is_just_released(&self, key: VirtualKeyCode) -> bool {
        self.0
            .get(&key)
            .map(|(last_frame, this_frame)| *last_frame && !*this_frame)
            .unwrap_or(false)
    }
}

pub struct App {
    audio_manager: AudioManager,
    state: Vec<Box<dyn GameState>>,
    keyboard: KeyboardState,

    fps_timer: f32,
    frames_counted: u32,
    fps: f32,
}

impl App {
    pub fn new(renderer: &render::Renderer) -> anyhow::Result<Self> {
        let audio_manager = AudioManager::<DefaultBackend>::new(Default::default())?;
        let bg_filename = "assets/song_select_bg.jpg";
        let bg_texture = Rc::new(Texture::from_file(bg_filename, renderer)?);

        let bg_sprite = Sprite::new(Rc::clone(&bg_texture), [0.0, 0.0, 0.0], renderer);

        let don_tex = Rc::new(Texture::from_file("assets/don.png", renderer)?);
        let kat_tex = Rc::new(Texture::from_file("assets/kat.png", renderer)?);
        let big_don_tex = Rc::new(Texture::from_file("assets/big_don.png", renderer)?);
        let big_kat_tex = Rc::new(Texture::from_file("assets/big_kat.png", renderer)?);

        let state = Box::new(SongSelect::new(
            bg_sprite,
            don_tex,
            kat_tex,
            big_don_tex,
            big_kat_tex,
        )?);

        Ok(App {
            audio_manager,
            state: vec![state],
            keyboard: KeyboardState(HashMap::new()),
            fps_timer: 0.0,
            frames_counted: 0,
            fps: 0.0,
        })
    }

    pub fn update(
        &mut self,
        delta: f32,
        renderer: &render::Renderer,
        control_flow: &mut ControlFlow,
    ) {
        self.fps_timer += delta;
        self.frames_counted += 1;

        if self.fps_timer >= FPS_POLL_TIME {
            self.fps = self.frames_counted as f32 / self.fps_timer;
            self.fps_timer = 0.0;
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
            StateTransition::Exit => control_flow.set_exit(),
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
                ui.label(
                    egui::RichText::new(format!("fps: {:.2}", self.fps))
                        .color(egui::Color32::from_rgb(255, 0, 255))
                        .size(20.0),
                );
            });
    }

    pub fn render<'a>(&'a mut self, ctx: &mut render::RenderContext<'a>) {
        self.state.last_mut().unwrap().render(ctx)
    }

    pub fn handle_event(&mut self, event: &WindowEvent<'_>) {
        // We make the current state handle input before the keyboard can update state,
        // so that the event is able to know what the state of the keyboard was before
        // the new input.
        self.state
            .last_mut()
            .unwrap()
            .handle_event(event, &self.keyboard);

        if let WindowEvent::KeyboardInput {
            input,
            is_synthetic: false,
            ..
        } = event
        {
            self.keyboard.handle_input(input)
        }
    }
}
