use crate::app::visual::button::Button;
use silkwood::{app::GameState, render::Renderer};

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

    fn debug_ui(&mut self, ctx: egui::Context, _audio: &mut kira::manager::AudioManager) {
        #[cfg(debug_assertions)]
        let build = "debug";

        #[cfg(not(debug_assertions))]
        let build = "release";

        egui::Area::new("version")
            .fixed_pos(egui::pos2(1700.0, 1050.0))
            .show(&ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "lunataiko version {} ({})",
                        env!("CARGO_PKG_VERSION"),
                        build,
                    ))
                    .color(egui::Color32::from_rgb(255, 255, 255))
                    .size(15.0),
                );
            });
    }
}
