#![allow(unused)]
//! Types for representing a song.

/// The type of a note. This includes a special note, which defines the end
/// of a drum roll. All drum rolls should be terminated with this note
/// (with the exception of the `SpecialRoll`, which can be terminated
/// with another `SpecialRoll`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteType {
    Don,
    Kat,
    BigDon,
    BigKat,
    Roll,
    BigRoll,
    BalloonRoll,
    RollEnd,
    SpecialRoll,
    BothDon,
    BothKa,
}

/// A struct representing a note, as it will be stored during the actual game.
/// A note has a type, the time (in seconds, from the song start) that it has
/// to be hit, and a constant speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    note_type: NoteType,
    time: f32,
    scroll_speed: f32,
}
