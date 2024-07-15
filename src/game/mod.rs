mod credits;
mod main_menu;
mod score_screen;
mod song_select;
mod taiko_mode;
mod ui_elements;

use kaku::{FontSize, HorizontalAlignment, Text, TextBuilder, VerticalAlignment};
pub use main_menu::MainMenu;
pub use song_select::SongSelect;

use std::rc::Rc;

use kira::manager::{backend::DefaultBackend, AudioManager};
use std::collections::HashMap;

use winit::{
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey},
};

use crate::render::{self, texture::Texture, Renderable, Renderer};

const FPS_POLL_TIME: f32 = 0.5;
const SPRITES_PATH: &str = "assets/images";

pub enum StateTransition {
    Continue,
    Push(Box<dyn GameState>),
    Swap(Box<dyn GameState>),
    Pop,
    Exit,
}

pub struct Context<'ctx> {
    pub audio: &'ctx mut AudioManager,
    pub renderer: &'ctx mut Renderer,
    pub keyboard: &'ctx KeyboardState,
    pub textures: &'ctx mut TextureCache,
    pub mouse: &'ctx MouseState,
}

pub struct RenderContext<'ctx, 'pass> {
    pub audio: &'ctx mut AudioManager,
    pub renderer: &'pass Renderer,
    pub textures: &'ctx mut TextureCache,
    pub keyboard: &'ctx KeyboardState,
    pub mouse: &'ctx MouseState,

    pub render_pass: &'ctx mut wgpu::RenderPass<'pass>,
}

impl<'pass> RenderContext<'_, 'pass> {
    pub fn render<R: Renderable>(&mut self, target: &'pass R) {
        target.render(self.renderer, self.render_pass);
    }
}

pub trait GameState {
    fn update(&mut self, _ctx: &mut Context, _delta_time: f32) -> StateTransition {
        StateTransition::Continue
    }

    // TODO: Fix this up.
    fn debug_ui(&mut self, _ctx: egui::Context, _audio: &mut AudioManager) {}

    fn render<'pass>(&'pass mut self, _ctx: &mut RenderContext<'_, 'pass>) {}

    fn handle_event(&mut self, _ctx: &mut Context, _event: &WindowEvent) {}
}

/// A struct that keeps track of the state of the keyboard at each frame.
///
/// Each keycode is mapped to a tuple containing two booleans; the first indicates whether the key
/// was pressed last frame, the second indicates whether the key is pressed this frame.
pub struct KeyboardState(HashMap<PhysicalKey, (bool, bool)>);

impl KeyboardState {
    fn handle_input(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        self.0.entry(event.physical_key).or_insert((false, false)).1 = pressed;
    }

    /// Returns whether or not the given key is pressed this frame.
    pub fn is_pressed(&self, key: PhysicalKey) -> bool {
        self.0.get(&key).is_some_and(|&(_, pressed)| pressed)
    }

    /// Returns whether or not the given key was just pressed this frame (i.e: pressed this frame
    /// but not last frame)
    pub fn is_just_pressed(&self, key: PhysicalKey) -> bool {
        self.0
            .get(&key)
            .is_some_and(|(last_frame, this_frame)| !(*last_frame) && *this_frame)
    }

    /// Returns whether or not the given key was just released this frame (i.e: released this frame
    /// but not last frame)
    pub fn is_just_released(&self, key: PhysicalKey) -> bool {
        self.0
            .get(&key)
            .is_some_and(|(last_frame, this_frame)| *last_frame && !*this_frame)
    }
}

pub struct MouseState {
    position: Option<(f32, f32)>,
    button_map: HashMap<MouseButton, (bool, bool)>,
}

impl MouseState {
    fn handle_input(&mut self, event: &WindowEvent) {
        match *event {
            WindowEvent::CursorMoved { position, .. } => {
                self.position = Some((position.x as f32, position.y as f32));
            }

            WindowEvent::CursorLeft { .. } => {
                self.position = None;
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;

                self.button_map.entry(button).or_insert((false, false)).1 = pressed;
            }

            _ => {}
        }
    }

    /// Returns whether or not the given button is pressed this frame.
    pub fn is_pressed(&self, button: MouseButton) -> bool {
        self.button_map
            .get(&button)
            .is_some_and(|&(_, pressed)| pressed)
    }

    /// Returns whether or not the given button was just pressed this frame (i.e: pressed this frame
    /// but not last frame)
    pub fn is_just_pressed(&self, button: MouseButton) -> bool {
        self.button_map
            .get(&button)
            .is_some_and(|(last_frame, this_frame)| !(*last_frame) && *this_frame)
    }

    /// Returns whether or not the given button was just released this frame (i.e: released this frame
    /// but not last frame)
    pub fn is_just_released(&self, button: MouseButton) -> bool {
        self.button_map
            .get(&button)
            .is_some_and(|(last_frame, this_frame)| *last_frame && !*this_frame)
    }

    pub fn cursor_pos(&self) -> Option<(f32, f32)> {
        self.position
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

pub struct Game {
    audio_manager: AudioManager,
    state: Vec<Box<dyn GameState>>,
    keyboard: KeyboardState,
    mouse: MouseState,
    textures: TextureCache,

    fps_timer: f32,
    frames_counted: u32,
    fps: f32,
    show_fps_counter: bool,

    version_text: Text,
}

impl Game {
    pub fn new<F>(renderer: &mut render::Renderer, create_state: F) -> anyhow::Result<Self>
    where
        F: FnOnce(&mut render::Renderer, &mut TextureCache) -> Box<dyn GameState>,
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

        #[cfg(debug_assertions)]
        let build = "debug";
        #[cfg(not(debug_assertions))]
        let build = "release";

        let version_text = format!(
            "luna's taiko sim - version {} ({})",
            env!("CARGO_PKG_VERSION"),
            build
        );

        let version_text = TextBuilder::new(version_text, renderer.font("mplus regular"), [1910., 1070.])
            .horizontal_align(HorizontalAlignment::Right)
            .vertical_align(VerticalAlignment::Bottom)
            .font_size(Some(FontSize::Px(16.)))
            .color([1.; 4])
            .outlined([0., 0., 0., 1.], 3.5)
            .build(&renderer.device, &renderer.queue, &mut renderer.text_renderer);

        Ok(Game {
            audio_manager,
            state: vec![state],
            keyboard: KeyboardState(HashMap::new()),
            mouse: MouseState {
                position: None,
                button_map: HashMap::new(),
            },
            textures,

            fps_timer: 0.0,
            frames_counted: 0,
            fps: 0.0,
            show_fps_counter: false,
            version_text,
        })
    }

    pub fn update(
        &mut self,
        delta: f32,
        renderer: &mut render::Renderer,
        event_loop: &ActiveEventLoop,
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
            mouse: &self.mouse,
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
            StateTransition::Exit => event_loop.exit(),
            StateTransition::Continue => {}
        }
    }

    pub fn debug_ui(&mut self, ctx: egui::Context) {
        self.state
            .last_mut()
            .unwrap()
            .debug_ui(ctx.clone(), &mut self.audio_manager);

        if self.show_fps_counter {
            egui::Area::new("fps counter".into())
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

    pub fn render<'pass>(
        &'pass mut self,
        renderer: &'pass Renderer,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        let mut ctx = RenderContext {
            audio: &mut self.audio_manager,
            renderer,
            keyboard: &self.keyboard,
            mouse: &self.mouse,
            textures: &mut self.textures,
            render_pass,
        };

        self.state.last_mut().unwrap().render(&mut ctx);
        ctx.render(&self.version_text);
    }

    pub fn handle_event(&mut self, event: &WindowEvent, renderer: &mut render::Renderer) {
        // We make the current state handle input before the keyboard can update state,
        // so that the event is able to know what the state of the keyboard was before
        // the new input.
        let mut ctx = Context {
            audio: &mut self.audio_manager,
            renderer,
            keyboard: &self.keyboard,
            mouse: &self.mouse,
            textures: &mut self.textures,
        };

        self.state.last_mut().unwrap().handle_event(&mut ctx, event);

        if let WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } = event
        {
            self.keyboard.handle_input(event);

            if self.keyboard.is_just_pressed(PhysicalKey::Code(KeyCode::F1)) {
                self.show_fps_counter = !self.show_fps_counter;
            }
        }

        self.mouse.handle_input(event);
    }
}
