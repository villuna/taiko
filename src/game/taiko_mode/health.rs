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

pub fn judgement_hp_values(difficulty: usize, star_rating: usize, num_notes: usize) -> [i32; 3] {
    // Magic formula
    let points_per_good = 13114. / num_notes as f32;
    // Okays give 0.5x the points of a good on oni difficulty, and 0.75x on all other difficulties.
    let okay_ratio = if matches!(difficulty, 3 | 4) {
        0.5
    } else {
        0.75
    };

    // Bads and misses take away points, also on a ratio of the points a good gives you. These
    // ratios are more complicated.
    // Values taken from https://wikiwiki.jp/taiko-fumen/%E3%82%B7%E3%82%B9%E3%83%86%E3%83%A0/%E9%AD%82%E3%82%B2%E3%83%BC%E3%82%B8%E3%81%AE%E4%BC%B8%E3%81%B3%E7%8E%87
    let bad_ratio = match difficulty {
        // Easy
        0 => -0.5,
        // Normal
        1 => match star_rating {
            1..=3 => -0.5,
            4 => -0.75,
            _ => -1.,
        },
        // Hard
        2 => match star_rating {
            1..=2 => -0.75,
            3 => -1.,
            4 => -7. / 6.,
            _ => -1.25,
        },
        // Oni
        _ => match star_rating {
            1..=7 => -1.6,
            _ => -2.,
        },
    };

    [
        points_per_good.round() as i32,
        (points_per_good * okay_ratio).round() as i32,
        (points_per_good * bad_ratio).round() as i32,
    ]
}
