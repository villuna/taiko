//! Functions and constants relating to how the health bar (or 'soul gauge') behaves.

pub fn clear_threshold(difficulty: usize) -> u32 {
    match difficulty {
        0 => 6000,
        1 | 2 => 7000,
        3 | 4 => 8000,
        // TODO: Use an enum instead of a usize to represent difficulty
        _ => panic!("Invalid difficulty level \"{difficulty}\""),
    }
}
