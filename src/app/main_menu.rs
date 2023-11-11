use silkwood::{app::GameState, render::Renderer, ui::button::Button};

pub struct MainMenu {
    test_button: Button,
}

impl MainMenu {
    pub fn new(renderer: &mut Renderer) -> Self {
        let test_button = Button::new(
            renderer,
            "Test Button",
            [900., 500.],
            [950., 135.],
            [224. / 255., 39. / 255., 50. / 255., 1.],
        )
        .unwrap();

        Self { test_button }
    }
}

impl GameState for MainMenu {
    fn render<'pass>(&'pass mut self, ctx: &mut silkwood::app::RenderContext<'_, 'pass>) {
        ctx.render(&self.test_button)
    }
}
