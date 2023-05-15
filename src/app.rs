use crate::{parser::parse_tja_file, track::Song};

pub struct App {
    test_track: Song,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let track_file = std::fs::read_to_string("example-tracks/Ready To/Ready to.tja")?;
        let test_track = parse_tja_file(&track_file)?;

        Ok(App { test_track })
    }

    pub fn update(&mut self, _delta: f32) {}

    pub fn debug_ui(&self, ctx: egui::Context) {
        egui::Window::new("taikotest").show(&ctx, |ui| {
            ui.label("It works!");
        });
    }
}
