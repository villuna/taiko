//! This module handles the glue between the windowing system winit and the rest of the
//! application.
use std::ops::Deref;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::error::OsError;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window, WindowId};

use crate::game::{Game, MainMenu};
use crate::render::Renderer;
use crate::settings;

struct TaikoAppInner {
    game: Game,
    renderer: Renderer,
}

pub struct TaikoApp {
    inner: Option<TaikoAppInner>,
    frame_time: Instant,
    delta: f32,
}

impl TaikoApp {
    pub fn new() -> Self {
        Self {
            inner: None,
            frame_time: Instant::now(),
            delta: 1. / 60.,
        }
    }
}

fn create_window(
    event_loop: &ActiveEventLoop,
    settings: impl Deref<Target = settings::Settings>,
) -> Result<Window, OsError> {
    let (resolution, fullscreen) = match settings.visual.resolution {
        settings::ResolutionState::BorderlessFullscreen => {
            (None, Some(Fullscreen::Borderless(None)))
        }
        settings::ResolutionState::Windowed(width, height) => {
            (Some(PhysicalSize::new(width, height)), None)
        }
        settings::ResolutionState::Fullscreen { .. } => {
            // TODO: support this i guess? I dunno
            todo!()
        }
    };

    let mut attributes = Window::default_attributes()
        .with_title("Unnamed taiko simulator!!")
        .with_fullscreen(fullscreen);

    if let Some(resolution) = resolution {
        attributes = attributes.with_inner_size(resolution);
    };

    event_loop.create_window(attributes)
}

impl ApplicationHandler for TaikoApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.inner.is_none() {
            let window =
                create_window(event_loop, settings::settings()).expect("Couldn't create window");
            // The window has to stay for the entire duration of the program so this is fine
            // just lets us get around wgpu's surface lifetime limitation
            let window = Box::leak(Box::new(window));
            let mut renderer = Renderer::new(window).expect("Couldn't construct renderer");
            let game = Game::new(&mut renderer, |renderer, textures| {
                Box::new(MainMenu::new(textures, renderer).unwrap())
            })
            .expect("Couldn't initialise game");

            self.inner = Some(TaikoAppInner { renderer, game });
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(TaikoAppInner {
            ref mut game,
            ref mut renderer,
        }) = self.inner
        else {
            return;
        };

        if !renderer.handle_event(&event) {
            game.handle_event(&event, renderer);

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }

                WindowEvent::Resized(size) => {
                    renderer.resize(size);
                }

                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(TaikoAppInner {
            ref mut game,
            ref mut renderer,
        }) = self.inner
        else {
            return;
        };

        game.update(self.delta, renderer, event_loop);
        match renderer.render(game) {
            Ok(_) => {}

            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                let size = renderer.size();
                renderer.resize(*size);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
            Err(e) => log::error!("error while rendering: {e:?}"),
        }

        let time = Instant::now();
        self.delta = time.duration_since(self.frame_time).as_secs_f32();
        self.frame_time = time;
    }
}
