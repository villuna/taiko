use std::rc::Rc;

use kira::manager::{backend::DefaultBackend, AudioManager};
use std::collections::HashMap;

use winit::{
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

use crate::render::{self, texture::Texture, context::Renderable};

const FPS_POLL_TIME: f32 = 0.5;
const SPRITES_PATH: &str = "assets/images";

pub enum StateTransition {
    Continue,
    Push(Box<dyn GameState>),
    Swap(Box<dyn GameState>),
    Pop,
    Exit,
}

pub struct Context<'app> {
    pub audio: &'app mut AudioManager,
    pub renderer: &'app mut render::Renderer,
    pub keyboard: &'app KeyboardState,
    pub textures: &'app mut TextureCache,
}

pub struct RenderContext<'app, 'pass> {
    pub audio: &'app mut AudioManager,
    pub render_pass: &'app mut render::RenderPassContext<'pass>,
    pub keyboard: &'app KeyboardState,
    pub textures: &'app mut TextureCache,
}

impl<'pass> RenderContext<'_, 'pass> {
    pub fn render<R: Renderable>(&mut self, target: &'pass R) {
        self.render_pass.render(target);
    }
}

pub trait GameState {
    fn update(&mut self, _ctx: &mut Context, _delta_time: f32) -> StateTransition {
        StateTransition::Continue
    }

    // TODO: Fix this up.
    fn debug_ui(&mut self, _ctx: egui::Context, _audio: &mut AudioManager) {}

    fn render<'app, 'pass>(&'pass mut self, _ctx: &mut RenderContext<'app, 'pass>) {}

    fn handle_event(&mut self, _ctx: &mut Context, _event: &WindowEvent<'_>) {}
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

#[derive(Default)]
pub struct TextureCache {
    cache: HashMap<&'static str, Rc<Texture>>,
}

impl TextureCache {
    pub fn get(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        filename: &'static str,
    ) -> anyhow::Result<Rc<Texture>> {
        match self.cache.get(&filename) {
            Some(tex) => Ok(Rc::clone(tex)),
            None => {
                let tex = Rc::new(Texture::from_file(
                    format!("{SPRITES_PATH}/{filename}"),
                    device,
                    queue,
                )?);
                self.cache.insert(filename, Rc::clone(&tex));
                Ok(tex)
            }
        }
    }
}

pub struct App {
    audio_manager: AudioManager,
    state: Vec<Box<dyn GameState>>,
    keyboard: KeyboardState,
    textures: TextureCache,

    fps_timer: f32,
    frames_counted: u32,
    fps: f32,
    show_fps_counter: bool,
}

impl App {
    pub fn new<F>(renderer: &mut render::Renderer, create_state: F) -> anyhow::Result<Self> 
    where F: FnOnce(&mut render::Renderer, &mut TextureCache) -> Box<dyn GameState>
    {
        let audio_manager = AudioManager::<DefaultBackend>::new(Default::default())?;
        let mut textures = TextureCache::default();
        // Let's load some important textures first
        for tex in [
            "don.png",
            "kat.png",
            "big_don.png",
            "big_kat.png",
            "drumroll_start.png",
            "big_drumroll_start.png",
        ] {
            textures
                .get(&renderer.device, &renderer.queue, tex)
                .unwrap();
        }

        let state = create_state(renderer, &mut textures);

        Ok(App {
            audio_manager,
            state: vec![state],
            keyboard: KeyboardState(HashMap::new()),
            textures,

            fps_timer: 0.0,
            frames_counted: 0,
            fps: 0.0,
            show_fps_counter: false,
        })
    }

    pub fn update(
        &mut self,
        delta: f32,
        renderer: &mut render::Renderer,
        control_flow: &mut ControlFlow,
    ) {
        self.fps_timer += delta;
        self.frames_counted += 1;

        if self.fps_timer >= FPS_POLL_TIME {
            self.fps = self.frames_counted as f32 / self.fps_timer;
            self.fps_timer = 0.0;
            self.frames_counted = 0;
        }

        let mut ctx = Context {
            audio: &mut self.audio_manager,
            renderer,
            keyboard: &self.keyboard,
            textures: &mut self.textures,
        };

        match self.state.last_mut().unwrap().update(&mut ctx, delta) {
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

        if self.show_fps_counter {
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
    }

    pub fn render<'pass>(&'pass mut self, rctx: &mut render::RenderPassContext<'pass>) {
        let mut ctx = RenderContext {
            audio: &mut self.audio_manager,
            render_pass: rctx,
            keyboard: &self.keyboard,
            textures: &mut self.textures,
        };

        self.state.last_mut().unwrap().render(&mut ctx)
    }

    pub fn handle_event(&mut self, event: &WindowEvent<'_>, renderer: &mut render::Renderer) {
        // We make the current state handle input before the keyboard can update state,
        // so that the event is able to know what the state of the keyboard was before
        // the new input.
        let mut ctx = Context {
            audio: &mut self.audio_manager,
            renderer,
            keyboard: &self.keyboard,
            textures: &mut self.textures,
        };

        self.state.last_mut().unwrap().handle_event(&mut ctx, event);

        if let WindowEvent::KeyboardInput {
            input,
            is_synthetic: false,
            ..
        } = event
        {
            self.keyboard.handle_input(input);

            if self.keyboard.is_just_pressed(VirtualKeyCode::F1) {
                self.show_fps_counter = !self.show_fps_counter;
            }
        }
    }
}
