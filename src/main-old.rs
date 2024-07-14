use std::{borrow::Borrow, ops::Deref, time::Instant};

use taiko::{
    app::{App, MainMenu},
    render::Renderer,
};

use taiko::settings::{self, SETTINGS};

use winit::{
    dpi::PhysicalSize, event::{Event, WindowEvent}, event_loop::{EventLoop, EventLoopWindowTarget}, window::{Fullscreen, Window, WindowBuilder}
};

fn build_window(event_loop: &EventLoopWindowTarget<()>, settings: impl Deref<Target = settings::Settings>) -> Window {
    let settings = settings.borrow();
    let (resolution, fullscreen) = match settings.visual.resolution {
        settings::ResolutionState::BorderlessFullscreen => {
            (None, Some(Fullscreen::Borderless(None)))
        },
        settings::ResolutionState::Windowed(width, height) => {
            (Some(PhysicalSize::new(width, height)), None)
        },
        settings::ResolutionState::Fullscreen { .. } => {
            // TODO: support this i guess? I dunno 
            todo!()
        },
    };

    let mut builder = WindowBuilder::new()
        .with_title("Unnamed taiko simulator!!")
        .with_fullscreen(fullscreen);

    if let Some(resolution) = resolution {
        builder = builder.with_inner_size(resolution);
    };

    builder.build(event_loop).unwrap()
}

fn main() {
    env_logger::init();

    settings::read_settings();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    // To be replaced when I update winit
    struct Everything {
        renderer: Renderer,
        app: App,
    }
    
    let mut frame_time = Instant::now();
    let mut delta = 1.0 / 60.0;
    let mut everything = None;
    
    event_loop.run(move |event, event_loop| {
        if everything.is_none() && matches!(event, Event::Resumed) {
            let window = build_window(event_loop, SETTINGS.read().unwrap());

            let window: &'static Window = Box::leak(Box::new(window));
            let mut renderer = Renderer::new(window).unwrap();
            let app = App::new(&mut renderer, |renderer, textures| {
                Box::new(MainMenu::new(textures, renderer).unwrap())
            })
            .unwrap();

            everything = Some(Everything {
                renderer,
                app,
            });

            return;
        }

        let Some(Everything { ref mut renderer, ref mut app }) = everything.as_mut() else { return; };

        if !renderer.handle_event(&event) {
            match event {
                Event::Resumed => {
                    
                },
                Event::WindowEvent { window_id, event } if window_id == renderer.window().id() => {
                    app.handle_event(&event, renderer);

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

                Event::AboutToWait => {
                    app.update(delta, renderer, event_loop);
                    match renderer.render(app) {
                        Ok(_) => {}

                        Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                            let size = renderer.size();
                            renderer.resize(*size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => log::error!("error while rendering: {e:?}"),
                    }

                    let time = Instant::now();
                    delta = time.duration_since(frame_time).as_secs_f32();
                    frame_time = time;
                }

                _ => {}
            }
        }
    }).unwrap();
}
