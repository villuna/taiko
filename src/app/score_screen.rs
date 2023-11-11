use kira::manager::AudioManager;

use silkwood::app::{Context, GameState, StateTransition};
struct Score {
    goods: u32,
    okays: u32,
    bads: u32,
    drumrolls: u32,
    max_combo: u32,
    // TODO: score, soul gauge
}

pub struct ScoreScreen {
    score: Score,
    song_name: String,
    exit: bool,
}

impl ScoreScreen {
    pub fn new(
        _ctx: &mut Context,
        goods: u32,
        okays: u32,
        bads: u32,
        drumrolls: u32,
        max_combo: u32,
        song_name: &str,
    ) -> Self {
        Self {
            score: Score {
                goods,
                okays,
                bads,
                drumrolls,
                max_combo,
            },

            song_name: song_name.to_string(),
            exit: false,
        }
    }
}

impl GameState for ScoreScreen {
    fn update(&mut self, _ctx: &mut Context, _delta_time: f32) -> StateTransition {
        if self.exit {
            StateTransition::Pop
        } else {
            StateTransition::Continue
        }
    }

    fn debug_ui(&mut self, ctx: egui::Context, _audio: &mut AudioManager) {
        egui::Window::new("Seiseki happyou!").show(&ctx, |ui| {
            ui.label(egui::RichText::new(&self.song_name).size(20.0).strong());
            ui.add_space(10.0);
            ui.label(format!("Good: {}", self.score.goods));
            ui.label(format!("Ok: {}", self.score.okays));
            ui.label(format!("Bad: {}", self.score.bads));
            ui.label("Drumroll: Not yet implemented :P");
            ui.label(format!("Max Combo: {}", self.score.max_combo));

            self.exit = ui.button("Back to menu").clicked();
        });
    }
}
