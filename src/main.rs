use std::time::Instant;

use app::App;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

#[tokio::main]
async fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
        .with_title("Taiko!!")
        .with_resizable(false)
        .build(&event_loop)
        .unwrap();

    let mut frame_time = Instant::now();

    let mut app = App::new(window).await.unwrap();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { window_id, event } if window_id == app.renderer.window().id() => {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    control_flow.set_exit();
                }

                WindowEvent::Resized(size) => {
                    app.renderer.resize(size);
                }

                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    app.renderer.resize(*new_inner_size);
                }

                _ => {}
            }
        }

        Event::RedrawRequested(window_id) if window_id == app.renderer.window().id() => {
            let time = Instant::now();
            let delta = time.duration_since(frame_time).as_secs_f32();
            frame_time = time;
            app.update(delta);
            match app.render() {
                Ok(_) => {}

                Err(wgpu::SurfaceError::Lost) => {
                    let size = app.renderer.size();
                    app.renderer.resize(*size);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => control_flow.set_exit(),
                Err(e) => log::error!("{e:?}"),
            }
        }

        Event::MainEventsCleared => app.renderer.window().request_redraw(),

        _ => {}
    });
}
