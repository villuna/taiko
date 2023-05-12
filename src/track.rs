#![allow(unused)]
//! Types for representing a song.
//!
//! The types in here so far are very minimal, and can't handle all of the
//! possible songs that are defined in taiko, yet. Eventually I plan on
//! accommodating these variations (such as diverging difficulty, different
//! tracks for different players etc).
//!
//! Note that times are generally represented in milliseconds. Unless specified,
//! that is the unit that time values will be in.

use std::collections::HashMap;

const DEFAULT_BPM: f32 = 120.0;

/// The type of a note.
///
/// This includes a special note, which defines the end
/// of a drum roll. All drum rolls should be terminated with this note
/// (with the exception of the [SpecialRoll][NoteType::SpecialRoll], which can be terminated
/// with another [SpecialRoll][NoteType::SpecialRoll]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, num_derive::FromPrimitive)]
pub enum NoteType {
    Don = 1,
    Kat,
    BigDon,
    BigKat,
    Roll,
    BigRoll,
    BalloonRoll,
    RollEnd,
    SpecialRoll,
    CoopDon,
    CoopKa,
}
/// A note, as it will be stored during the actual game.
///
/// A note has a type, the time (from the song start) that it has
/// to be hit on, and a constant speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub note_type: NoteType,
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
    /// The amount of time between the beginning of the track and the first measure,
    /// measured in *seconds*.
    /// note timing will be relative to this offset
    pub offset: f32,
    /// The time *in seconds* that the song preview should start from.
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

/// A single difficulty setting and its associated track.
///
/// TODO: currently this cannot handle "Diverge Notes". see [NoteTrack]
/// for details. It also cannot handle multiple tracks for different
/// players.
#[derive(Debug, Clone)]
pub struct Difficulty {
    pub star_level: u8,
    pub track: NoteTrack,
}

/// The notes for a single difficulty setting.
///
/// TODO: Currently, this is just a linear stream of notes. Eventually
/// we will have to handle songs with multiple streams that switch
/// depending on the player's performance ("diverge notes").
#[derive(Default, Debug, Clone)]
pub struct NoteTrack {
    pub notes: Vec<Note>,
    pub balloons: Vec<u16>,
    pub measures: Vec<f32>,
}
