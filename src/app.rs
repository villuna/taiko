// Nothing?? for now...
pub struct App;

impl App {
    pub fn new() -> anyhow::Result<Self> {
        Ok(App)
    }

    pub fn update(&mut self, _delta: f32) {}

    pub fn debug_ui(&self, ctx: egui::Context) {
        egui::Window::new("taikotest").show(&ctx, |ui| {
            ui.label("It works!");
        });
    }
}
