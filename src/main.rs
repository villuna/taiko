mod settings;
mod beatmap_parser;
mod render;
mod game;
mod app;

use winit::event_loop::EventLoop;
use app::TaikoApp;

fn main() {
    settings::read_settings();

    let event_loop = EventLoop::new().expect("Couldn't construct window event loop!");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut TaikoApp::new()).unwrap()
}
