use egui::RichText;
use kira::manager::AudioManager;

use silkwood::app::{self, GameState, StateTransition};

pub struct CreditsScreen {
    exit: bool,
}

impl CreditsScreen {
    pub fn new() -> Self {
        Self { exit: false }
    }
}

impl GameState for CreditsScreen {
    fn update(&mut self, _ctx: &mut app::Context, _dt: f32) -> StateTransition {
        if self.exit {
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }
    fn debug_ui(&mut self, ctx: egui::Context, _audio: &mut AudioManager) {
        egui::Area::new("Credits")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(&ctx, |ui| {
                // Main credits
                ui.label(RichText::new("Made with love by:").size(50.0));
                ui.label(RichText::new("villi aka luna").size(30.0));

                ui.add_space(100.0);

                if ui.button(RichText::new("return").size(20.0)).clicked() {
                    self.exit = true;
                }
            });
    }
}
