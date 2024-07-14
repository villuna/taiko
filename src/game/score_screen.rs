use kira::manager::AudioManager;

use crate::game::taiko_mode::PlayResult;
use crate::game::{Context, GameState, StateTransition};

struct Score {
    // Some precomputed values to display
    goods: usize,
    okays: usize,
    bads: usize,
    max_combo: usize,
    drumrolls: u64,
}

impl Score {
    fn from_result(result: &PlayResult) -> Self {
        Self {
            goods: result.goods(),
            okays: result.okays(),
            bads: result.bads() + result.misses(),
            drumrolls: result.drumrolls(),
            max_combo: result.max_combo(),
        }
    }
}

pub struct ScoreScreen {
    score: Score,
    song_name: String,
    exit: bool,
}

impl ScoreScreen {
    pub fn new(_ctx: &mut Context, song_name: String, result: PlayResult) -> Self {
        Self {
            score: Score::from_result(&result),
            song_name,
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
        egui::Window::new("Let's see your results!").show(&ctx, |ui| {
            ui.label(egui::RichText::new(&self.song_name).size(20.0).strong());
            ui.add_space(10.0);
            ui.label(format!("Good: {}", self.score.goods));
            ui.label(format!("Ok: {}", self.score.okays));
            ui.label(format!("Bad: {}", self.score.bads));
            ui.label(format!("Drumrolls: {}", self.score.drumrolls));
            ui.label(format!("Max Combo: {}", self.score.max_combo));

            self.exit = ui.button("Back to menu").clicked();
        });
    }
}
