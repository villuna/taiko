// Nothing?? for now...
pub struct App;

impl App {
    pub fn new() -> anyhow::Result<Self> {
        Ok(App)
    }

    pub fn update(&mut self, _delta: f32) {}
}
