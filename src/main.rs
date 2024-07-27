mod app;
mod game;
mod notechart_parser;
mod render;
mod settings;

use app::TaikoApp;
use winit::event_loop::EventLoop;

fn main() {
    settings::read_settings();

    let event_loop = EventLoop::new().expect("Couldn't construct window event loop!");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut TaikoApp::new()).unwrap()
}
