#![allow(unused)]
//! Types for representing a song.
//!
//! The types in here so far are very minimal, and can't handle all of the
//! possible songs that are defined in taiko, yet. Eventually I plan on
//! accommodating these variations (such as diverging difficulty, different
//! tracks for different players etc).
//!
//! Note that times are generally represented in seconds. Unless specified,
//! that is the unit the time values will be in.

use std::collections::HashMap;

const DEFAULT_BPM: f32 = 120.0;

/// The type of note (e.g., Don, Ka, Balloon etc)
///
/// Drumroll variants also contain a float value indicating how long the drumroll continues for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteType {
    Don,
    Kat,
    BigDon,
    BigKat,
    Roll(f32),
    BigRoll(f32),
    BalloonRoll(f32, u32),
    SpecialRoll(f32, u32),
    CoopDon,
    CoopKat,
}

impl NoteType {
    pub fn is_roll(&self) -> bool {
        matches!(
            self,
            NoteType::Roll(_)
                | NoteType::BigRoll(_)
                | NoteType::BalloonRoll(_, _)
                | NoteType::SpecialRoll(_, _)
        )
    }

    pub fn is_don(&self) -> bool {
        matches!(self, NoteType::Don | NoteType::BigDon | NoteType::CoopDon)
    }

    pub fn is_kat(&self) -> bool {
        matches!(self, NoteType::Kat | NoteType::BigKat | NoteType::CoopKat)
    }
}

/// A note, as it will be stored during the actual game.
///
/// A note has a type, the time (from the song start) that it has
/// to be hit on, and a constant speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub note_type: NoteType,
    pub time: f32,
    /// The scroll speed as a multiple of the default speed.
    ///
    /// Default speed is such that at 120bpm, exactly one bar of notes is displayed on the screen.
    /// This will automatically be scaled with frame rate, so default scroll for notes at 240bpm
    /// will be 2.0.
    pub scroll_speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Barline {
    pub time: f32,
    pub scroll_speed: f32,
}

/// The data for a song, including its metadata and difficulties/note tracks.
#[derive(Debug, Clone)]
pub struct Song {
    pub title: String,
    pub subtitle: Option<String>,
    pub audio_filename: String,
    pub bpm: f32,
    /// The offset of the notes in seconds.
    /// This is the number of seconds earlier notes should appear relative to the song. i.e., if the
    /// offset is positive, notes will appear earlier. If it is negative, they will appear later.
    pub offset: f32,
    /// The time that the song preview should start from.
    pub demostart: f32,
    pub difficulties: [Option<Difficulty>; 5],
}

impl Default for Song {
    fn default() -> Self {
        Self {
            title: "".to_string(),
            subtitle: None,
            audio_filename: "".to_string(),
            bpm: DEFAULT_BPM,
            offset: 0.0,
            demostart: 0.0,
            difficulties: [None, None, None, None, None],
        }
    }
}

/// A single difficulty setting and its associated chart.
///
/// TODO: currently this cannot handle "Diverge Notes". see [NoteChart]
/// for details. It also cannot handle multiple tracks for different
/// players.
#[derive(Debug, Clone)]
pub struct Difficulty {
    pub star_level: u8,
    /// The score the player gets for a Good
    pub base_score: u32,
    pub chart: NoteChart,
}

/// The notes for a single difficulty setting.
///
/// TODO: Currently, this is just a linear stream of notes. Eventually
/// we will have to handle songs with multiple streams that switch
/// depending on the player's performance ("diverge notes").
#[derive(Default, Debug, Clone)]
pub struct NoteChart {
    pub notes: Vec<Note>,
    pub barlines: Vec<Barline>,
}

impl NoteChart {
    /// The maximum combo possible in this chart.
    /// That is to say, the number of notes in the chart that are don or ka
    pub fn max_combo(&self) -> usize {
        self.notes
            .iter()
            .filter(|note| note.note_type.is_don() || note.note_type.is_kat())
            .count()
    }
}
