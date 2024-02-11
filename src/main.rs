use std::time::Instant;

use taiko::{app::{MainMenu, App}, render::Renderer};

use taiko::settings::{self, ResolutionState, SETTINGS};

use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Fullscreen, Window, WindowBuilder},
};

fn set_window_mode(window: &Window, settings: &mut settings::Settings) {
    match settings.visual.resolution {
        settings::ResolutionState::BorderlessFullscreen => {
            let default_resolution = window.current_monitor().and_then(|monitor| {
                let size = monitor.size();

                if size.width != 0 && size.height != 0 {
                    Some((size.width, size.height))
                } else {
                    None
                }
            });

            match default_resolution {
                Some((width, height)) => {
                    window.set_inner_size(PhysicalSize::new(width, height));
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                }

                None => {
                    // Use default window resolution
                    let current_resolution = window.inner_size();
                    settings.visual.resolution = ResolutionState::Windowed(
                        current_resolution.width,
                        current_resolution.height,
                    );
                    log::error!("Couldn't set window to borderless fullscreen");
                }
            }
        }
        settings::ResolutionState::Windowed(width, height) => {
            window.set_fullscreen(None);
            window.set_inner_size(PhysicalSize::new(width, height));
        }
        settings::ResolutionState::Fullscreen { .. } => {
            let video_mode = window
                .current_monitor()
                .and_then(|monitor| monitor.video_modes().next());

            match video_mode {
                Some(mode) => {
                    window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                }

                None => {
                    // Use default window resolution
                    let current_resolution = window.inner_size();
                    settings.visual.resolution = ResolutionState::Windowed(
                        current_resolution.width,
                        current_resolution.height,
                    );
                    log::error!("Couldn't set window to exclusive fullscreen");
                }
            }
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("LunaTaiko!!")
        .with_inner_size(PhysicalSize::new(1920, 1080))
        .build(&event_loop)
        .unwrap();

    settings::read_settings();

    set_window_mode(&window, &mut SETTINGS.write().unwrap());

    let mut frame_time = Instant::now();
    let mut delta = 1.0 / 60.0;

    let mut renderer = Renderer::new(window).unwrap();

    let mut app = App::new(&mut renderer, |renderer, textures| {
        Box::new(MainMenu::new(textures, renderer).unwrap())
    })
    .unwrap();

    event_loop.run(move |event, _, control_flow| {
        if !renderer.handle_event(&event) {
            match event {
                Event::WindowEvent { window_id, event } if window_id == renderer.window().id() => {
                    app.handle_event(&event, &mut renderer);

                    match event {
                        WindowEvent::CloseRequested => {
                            control_flow.set_exit();
                        }

                        WindowEvent::Resized(size) => {
                            renderer.resize(size);
                        }

                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            renderer.resize(*new_inner_size);
                        }

                        _ => {}
                    }
                }

                Event::RedrawRequested(window_id) if window_id == renderer.window().id() => {
                    app.update(delta, &mut renderer, control_flow);
                    match renderer.render(&mut app) {
                        Ok(_) => {}

                        Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                            let size = renderer.size();
                            renderer.resize(*size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => control_flow.set_exit(),
                        Err(e) => log::error!("error while rendering: {e:?}"),
                    }

                    let time = Instant::now();
                    delta = time.duration_since(frame_time).as_secs_f32();
                    frame_time = time;
                }

                Event::MainEventsCleared => renderer.window().request_redraw(),

                _ => {}
            }
        }
    });
}
