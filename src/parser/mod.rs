//! Functions for parsing tja files into beatmaps.
//!
//! This module uses [nom] for parsing, so contains a lot of private parsing functinos. Where necessary, I've tried my best to annotate them with a grammar, written in (some kind of) EBNF form with embedded regex.
//! Grammar format key:
//!
//! ```text
//! symbol1 ::= symbol2 "raw string" /regex/ [optional component] {repeated component}
//!
//! symbol2 ::= alternative1 | alternative2
//!
//! embeddedExpression ::= "string with embedded expression: {1 + 2 * i}" ```

use std::collections::HashMap;

use crate::track::{Difficulty, Note, NoteTrack, NoteType, Song};
use itertools::Itertools;
use lookahead::Lookahead;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::{crlf, newline};
use nom::character::complete::{multispace0, satisfy};
use nom::combinator::recognize;
use nom::combinator::{eof, map_res, opt};
use nom::error::{FromExternalError, ParseError};
use nom::multi::{many0, many0_count, separated_list0};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated};
use nom::{Finish, IResult, Parser};

mod test;
mod tja_parser;

/// An enum that represents a single unit of information in a TJA file.
/// This is either a metadata tag, or an entire specification of the notes in
/// a difficulty. A TJA file is nothing more than a list of these items
/// separated by lines.
#[derive(Debug, Clone, PartialEq)]
enum TJAFileItem<'a> {
    Metadata(&'a str, &'a str),
    NoteTrack(Vec<NoteTrackEntry<'a>>),
}

/// The type of a note.
///
/// This includes a special note, which defines the end
/// of a drum roll. All drum rolls should be terminated with this note
/// (with the exception of the [SpecialRoll][NoteType::SpecialRoll], which can be terminated
/// with another [SpecialRoll][NoteType::SpecialRoll]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, num_derive::FromPrimitive)]
pub enum TJANoteType {
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
    CoopKat,
}

#[derive(Debug, Clone, PartialEq)]
enum NoteTrackEntry<'a> {
    Command(TrackCommand<'a>),
    Notes(Vec<Option<TJANoteType>>),
    EndMeasure,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Player {
    Player1,
    Player2,
}

#[derive(Debug, Clone, PartialEq)]
enum TrackCommand<'a> {
    Start { player: Option<Player> },
    Lyric(&'a str),
    BpmChange(f32),
    Measure(u8, u8),
    Delay(f32),
    Scroll(f32),
    GogoStart,
    GogoEnd,
    BarlineOff,
    BarlineOn,
    // TODO: Commands for diverge notes
}

/// Errors associated with parsing TJA files
#[derive(Debug, PartialEq, Eq)]
pub enum TJAParseError<I> {
    NomError {
        input: I,
        kind: nom::error::ErrorKind,
    },
    TrackCommandError,
    UnexpectedEndCommand,
    InvalidNote(char),
    RollEndWithoutRoll,
    RollNotEnded,
    NoteTrackNotEnded,
    MultipleTracksSameDifficulty(usize),
    InvalidMetadata(I, I),
    MetadataNeeded(I),
}

impl<'a> TJAParseError<&'a str> {
    fn into_nom_error(self) -> nom::Err<Self> {
        match self {
            // Recoverable errors:
            TJAParseError::NomError { .. }
            | TJAParseError::UnexpectedEndCommand
            | TJAParseError::InvalidNote(_) => nom::Err::Error(self),

            // Unrecoverable errors:
            _ => nom::Err::Failure(self),
        }
    }
}

impl<'a> ParseError<&'a str> for TJAParseError<&'a str> {
    fn from_error_kind(input: &'a str, kind: nom::error::ErrorKind) -> Self {
        TJAParseError::NomError { input, kind }
    }

    fn append(_: &str, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<'a, E> FromExternalError<&'a str, E> for TJAParseError<&'a str> {
    fn from_external_error(input: &'a str, kind: nom::error::ErrorKind, _e: E) -> Self {
        Self::from_error_kind(input, kind)
    }
}

impl<I: std::fmt::Display> std::fmt::Display for TJAParseError<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error_str = match self {
            TJAParseError::NomError { input, kind } => {
                format!("syntax error at input: \"{input}\" ({kind:?})")
            }
            TJAParseError::TrackCommandError => "error while parsing track command".to_string(),
            TJAParseError::UnexpectedEndCommand => "unexpected #END command".to_string(),
            TJAParseError::InvalidNote(c) => format!("invalid note: {c}"),
            TJAParseError::NoteTrackNotEnded => "note track not ended".to_string(),
            TJAParseError::RollEndWithoutRoll => {
                "roll end note (8) placed without preceding drumroll".to_string()
            }
            TJAParseError::RollNotEnded => "drumroll not ended".to_string(),
            TJAParseError::MultipleTracksSameDifficulty(diff) => {
                let diff_str = match diff {
                    0 => "easy",
                    1 => "normal",
                    2 => "hard",
                    3 => "oni",
                    4 => "ura oni",
                    _ => "unknown difficulty",
                };

                format!("multiple tracks with the same difficulty ({})", diff_str)
            }
            TJAParseError::InvalidMetadata(name, value) => {
                format!("invalid metadata given: \"{name}: {value}\"")
            }
            TJAParseError::MetadataNeeded(name) => format!("metadata missing: {name}"),
        };

        f.write_str(&error_str)
    }
}

impl<I: std::fmt::Display + std::fmt::Debug> std::error::Error for TJAParseError<I> {}

// This exists so we can return an owned version of the errors when we need to (for example, after
// preprocessing the input into a string owned by the function, then returning a result that
// references that string).
impl From<TJAParseError<&str>> for TJAParseError<String> {
    fn from(value: TJAParseError<&str>) -> Self {
        match value {
            TJAParseError::NomError { input, kind } => TJAParseError::NomError {
                input: input.to_string(),
                kind,
            },
            TJAParseError::InvalidMetadata(name, value) => {
                TJAParseError::InvalidMetadata(name.to_string(), value.to_string())
            }
            TJAParseError::MetadataNeeded(name) => TJAParseError::MetadataNeeded(name.to_string()),
            TJAParseError::InvalidNote(c) => TJAParseError::InvalidNote(c),
            TJAParseError::TrackCommandError => TJAParseError::TrackCommandError,
            TJAParseError::NoteTrackNotEnded => TJAParseError::NoteTrackNotEnded,
            TJAParseError::UnexpectedEndCommand => TJAParseError::UnexpectedEndCommand,
            TJAParseError::MultipleTracksSameDifficulty(diff) => {
                TJAParseError::MultipleTracksSameDifficulty(diff)
            }
            TJAParseError::RollEndWithoutRoll => TJAParseError::RollEndWithoutRoll,
            TJAParseError::RollNotEnded => TJAParseError::RollNotEnded,
        }
    }
}

impl<'a> TrackCommand<'a> {
    /// Creates a new "inner track command" (that is one that isn't START or END), from name and
    /// value
    fn inner_from_name_arg(
        name: &'a str,
        arg: Option<&'a str>,
    ) -> Result<Self, TJAParseError<&'a str>> {
        // TODO: parse more commands
        use TJAParseError::TrackCommandError;
        let arg_res = arg.ok_or(TrackCommandError);

        let command = match name {
            "END" => return Err(TJAParseError::UnexpectedEndCommand),
            "LYRIC" => TrackCommand::Lyric(arg_res?),
            "BPMCHANGE" => {
                TrackCommand::BpmChange(arg_res?.parse::<f32>().map_err(|_| TrackCommandError)?)
            }
            "MEASURE" => {
                let (_, (numerator, denominator)) =
                    parse_time_signature(arg_res?).map_err(|_| TrackCommandError)?;

                TrackCommand::Measure(numerator, denominator)
            }
            "DELAY" => TrackCommand::Delay(arg_res?.parse::<f32>().map_err(|_| TrackCommandError)?),
            "SCROLL" => {
                TrackCommand::Scroll(arg_res?.parse::<f32>().map_err(|_| TrackCommandError)?)
            }
            "GOGOSTART" | "GOGOEND" | "BARLINEOFF" | "BARLINEON" => {
                // These dont take any arguments, so ensure there is no arg
                if arg.is_some() {
                    return Err(TrackCommandError);
                }

                match name {
                    "GOGOSTART" => TrackCommand::GogoStart,
                    "GOGOEND" => TrackCommand::GogoEnd,
                    "BARLINEOFF" => TrackCommand::BarlineOff,
                    "BARLINEON" => TrackCommand::BarlineOn,
                    _ => unreachable!(),
                }
            }
            _ => return Err(TrackCommandError),
        };

        Ok(command)
    }
}

// --- Parsing helper functions ---

/// Parses a single uppercase letter.
fn uppercase(i: &str) -> IResult<&str, char, TJAParseError<&str>> {
    satisfy(|c| c.is_ascii_uppercase())(i)
}

/// Parses a single digit from 0 to 9.
fn digit(i: &str) -> IResult<&str, char, TJAParseError<&str>> {
    satisfy(|c| c.is_ascii_digit())(i)
}

// Parses an empty line, i.e. whitespace terminated by a newline
fn empty_line<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&str, (), E> {
    terminated(
        many0_count(satisfy(|c| c != '\n' && c.is_whitespace())),
        newline,
    )
    .map(|_| ())
    .parse(i)
}

// Takes a parser that returns some value and turns it into a parser that
// returns `()`. Useful for when you have multiple parsers whose results
// you don't care about but you still need them to have the same type
// signature.
fn toss<'a, F, O, E: ParseError<&'a str>>(
    mut inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, (), E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    move |i| {
        let (next, _) = inner(i)?;

        Ok((next, ()))
    }
}

// Matches an object on its own line, surrounded by any number of
// empty lines.
fn line_of<'a, F, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    let newline_or_eof = alt((toss(crlf), toss(newline), toss(eof)));
    delimited(
        many0_count(empty_line),
        terminated(inner, newline_or_eof),
        many0_count(empty_line),
    )
}

// --- Parsing functions for metadata ---

// Parses a metadata tag, which will be an identifier that consists only of
// uppercase letters and numbers. It must start with an uppercase letter.
//
// metadata_tagname ::= /[A-Z][A-Z0-9]*/
fn metadata_tagname(i: &str) -> IResult<&str, &str, TJAParseError<&str>> {
    recognize(pair(uppercase, many0_count(alt((uppercase, digit)))))(i)
}

// Parses a colon and an optional space, and throws away the result
//
// colon_separator ::= ":" [" "]
fn colon_separator(i: &str) -> IResult<&str, (), TJAParseError<&str>> {
    toss(pair(tag(":"), opt(tag(" "))))(i)
}

// Parses a single metadata definition: a line containing a name for the attribute, followed by a
// colon and optional space, then the value.
//
// metadata_pair ::= metadata_tagname colon_separator /[^\n\r]*/
fn metadata_pair(i: &str) -> IResult<&str, (&str, &str), TJAParseError<&str>> {
    line_of(separated_pair(
        metadata_tagname,
        colon_separator,
        opt(is_not("\n\r")).map(|s| s.unwrap_or("")),
    ))(i)
}

fn integer<T: std::str::FromStr>(i: &str) -> IResult<&str, T, TJAParseError<&str>> {
    let onenine = satisfy(|c| ('1'..'9').contains(&c));

    map_res(
        alt((
            recognize(pair(onenine, many0_count(digit))),
            recognize(digit),
        )),
        |i_str: &str| i_str.parse::<T>(),
    )(i)
}

// --- Parsing functions for tracks ---
fn parse_time_signature(i: &str) -> IResult<&str, (u8, u8), TJAParseError<&str>> {
    terminated(separated_pair(integer::<u8>, tag("/"), integer::<u8>), eof)(i)
}

fn start_command(i: &str) -> IResult<&str, TrackCommand, TJAParseError<&str>> {
    let (i, opt_player) = line_of(preceded(
        tag("#START"),
        opt(preceded(tag(" "), is_not("\n\r"))),
    ))(i)?;

    let player = match opt_player {
        None => None,
        Some("P1") => Some(Player::Player1),
        Some("P2") => Some(Player::Player2),
        _ => return Err(nom::Err::Failure(TJAParseError::TrackCommandError)),
    };
    Ok((i, TrackCommand::Start { player }))
}

fn end_command(i: &str) -> IResult<&str, (), TJAParseError<&str>> {
    toss(line_of(tag("#END")))(i)
}

// Parses a track command (that isn't #START or #END), converting the result into a [TrackCommand] enum type.
//
// See [track_command_raw] for more details.
fn inner_track_command(i: &str) -> IResult<&str, TrackCommand, TJAParseError<&str>> {
    let (i, (name, arg)) = track_command_raw(i)?;
    TrackCommand::inner_from_name_arg(name, arg)
        .map(|command| (i, command))
        .map_err(|e| e.into_nom_error())
}

// Parses a track command, returning the name and value as raw strings.
//
// A track command has a tag name and optionally, a value.
// Returns a tuple containing the name and optional value.
//
// track_command ::= '#' metadata_tagname [' ' /[^\n\r]/]
fn track_command_raw(i: &str) -> IResult<&str, (&str, Option<&str>), TJAParseError<&str>> {
    line_of(preceded(
        tag("#"),
        pair(metadata_tagname, opt(preceded(tag(" "), is_not("\n\r")))),
    ))(i)
}

fn note(i: &str) -> IResult<&str, Option<TJANoteType>, TJAParseError<&str>> {
    satisfy(|c| c.is_ascii_digit() || ['A', 'B'].contains(&c))
        .map(|c| {
            if c == '0' {
                None
            } else if let Some(code) = c.to_digit(12) {
                num_traits::FromPrimitive::from_u32(code)
            } else {
                unreachable!()
            }
        })
        .parse(i)
}

fn notes(i: &str) -> IResult<&str, Vec<Option<TJANoteType>>, TJAParseError<&str>> {
    many0(note)(i)
}

fn note_track_inner(mut i: &str) -> IResult<&str, Vec<NoteTrackEntry>, TJAParseError<&str>> {
    let mut res = Vec::new();

    while eof::<_, nom::error::Error<_>>(i).is_err() {
        if end_command(i).is_ok() {
            return Ok((i, res));
        }

        let (new_i, entry) = alt((
            inner_track_command.map(NoteTrackEntry::Command),
            pair(tag(","), alt((tag("\n"), tag("\r\n")))).map(|_| NoteTrackEntry::EndMeasure),
            preceded(multispace0, notes.map(NoteTrackEntry::Notes)),
        ))(i)?;

        res.push(entry);
        i = new_i;
    }

    Err(nom::Err::Failure(TJAParseError::NoteTrackNotEnded))
}

fn note_track(i: &str) -> IResult<&str, Vec<NoteTrackEntry>, TJAParseError<&str>> {
    terminated(
        pair(start_command.map(NoteTrackEntry::Command), note_track_inner),
        end_command,
    )
    .map(|(start, mut inner)| {
        inner.insert(0, start);
        inner
    })
    .parse(i)
}

// --- Parsing for the file as a whole ---

fn tja_item(i: &str) -> IResult<&str, TJAFileItem, TJAParseError<&str>> {
    alt((
        metadata_pair.map(|(tag, value)| TJAFileItem::Metadata(tag, value)),
        note_track.map(TJAFileItem::NoteTrack),
    ))(i)
}

fn tja_file(i: &str) -> IResult<&str, Vec<TJAFileItem>, TJAParseError<&str>> {
    terminated(many0(tja_item), eof)(i)
}

/// Strips away the comments from a tja file, and removes the zero width byte alignment character
/// from the front if it exists.
fn preprocess_tja_file(input: &str) -> String {
    Itertools::intersperse(
        input
            .lines()
            // Get rid of zero width spaces, which for some reason keep starting the tja files I
            // test.
            .map(|line| line.strip_prefix('\u{feff}').unwrap_or(line))
            // Remove comments
            .map(|line| line.find("//").map(|i| &line[0..i]).unwrap_or(line)),
        "\n",
    )
    .collect()
}

pub fn tja_file_bench(i: &str) {
    let no_comments = preprocess_tja_file(i);
    tja_file(&no_comments).finish().map(|_| ()).unwrap();
}

fn get_parsed_metadata<'a, T: std::str::FromStr>(
    metadata: &HashMap<&'a str, &'a str>,
    key: &'a str,
    default: Option<T>,
) -> Result<T, TJAParseError<&'a str>> {
    metadata
        .get(key)
        .map(|s| {
            s.parse::<T>()
                .map_err(|_| TJAParseError::InvalidMetadata(key, s))
        })
        .unwrap_or(match default {
            Some(t) => Ok(t),
            None => Err(TJAParseError::MetadataNeeded(key)),
        })
}

fn get_metadata_owned<'a>(
    metadata: &HashMap<&'a str, &'a str>,
    key: &'a str,
    default: Option<&str>,
) -> Result<String, TJAParseError<&'a str>> {
    let value = if let Some(s) = metadata.get(key) {
        s.to_string()
    } else {
        match default {
            Some(s) => s.to_string(),
            None => return Err(TJAParseError::MetadataNeeded(key)),
        }
    };

    Ok(value)
}

/// Calculate the number of notes between now and the end of the current measure.
///
/// Since this number is used for timing calculations, the number includes empty notes that are
/// used in the tja format for timing.
///
/// Requires the argument to be a [lookahead::Lookahead] as we need to be able to walk through
/// the rest of the iterator to count the notes.
fn notes_in_next_measure<'a, I: Iterator<Item = &'a NoteTrackEntry<'a>>>(
    iter: &mut Lookahead<I>,
) -> usize {
    let mut num_notes = 0;

    for i in 0.. {
        let Some(item) = iter.lookahead(i) else { break; };

        match item {
            NoteTrackEntry::Notes(notes) => num_notes += notes.len(),
            NoteTrackEntry::EndMeasure => break,
            _ => {}
        }
    }

    num_notes
}

fn construct_difficulty<'a>(
    track_items: &[NoteTrackEntry<'a>],
    metadata: &HashMap<&'a str, &'a str>,
    difficulties: &mut [Option<Difficulty>; 5],
) -> Result<(), TJAParseError<&'a str>> {
    // Start by seeing what metadata is currently set.
    // Since course metadata and song metadata can be freely mixed, we have to
    // check the metadata every time before making a new track.

    // See what difficulty the track should be.
    let difficulty_level = match metadata.get("COURSE") {
        Some(&course) => match course {
            "Easy" | "0" => 0,
            "Normal" | "1" => 1,
            "Hard" | "2" => 2,
            "Oni" | "3" => 3,
            "Edit" | "4" => 4,
            _ => return Err(TJAParseError::InvalidMetadata("COURSE", course)),
        },

        // Default difficulty is oni
        None => 3,
    };

    // Ensure there isn't a track already defined with this difficulty
    if difficulties[difficulty_level].is_some() {
        return Err(TJAParseError::MultipleTracksSameDifficulty(
            difficulty_level,
        ));
    }

    let mut track = NoteTrack::default();

    // Various metadata needed for constructing the track
    // The time signature, as numerator divided by denominator (musicians might
    // disagree but I feel pretty good about it)
    // Defaults to common time
    let mut signature = 1f32;
    const DEFAULT_BPM: f32 = 120.0;
    let mut bpm = get_parsed_metadata::<f32>(metadata, "BPM", Some(DEFAULT_BPM))?;
    let offset = get_parsed_metadata::<f32>(metadata, "OFFSET", Some(0.0))?;
    let init_scroll_speed = get_parsed_metadata::<f32>(metadata, "HEADSCROLL", Some(1.0))?;
    let mut unscaled_scroll = init_scroll_speed;
    let mut scroll_speed = init_scroll_speed * bpm / DEFAULT_BPM;
    let mut balloon_count = 0;

    let mut items_iter = lookahead::lookahead(track_items);
    let mut notes_in_measure = notes_in_next_measure(&mut items_iter);
    let mut seconds_per_measure = 60.0 * signature * 4.0 / bpm;

    let mut seconds_per_note = if notes_in_measure == 0 {
        0.0
    } else {
        seconds_per_measure / notes_in_measure as f32
    };

    let mut time = -offset;
    let mut measure_start_time = time;
    let mut barlines = vec![time];
    let mut barline_on = true;

    let mut notes = Vec::new();

    while let Some(item) = items_iter.next() {
        match item {
            NoteTrackEntry::Command(command) => match command {
                TrackCommand::BpmChange(new_bpm) => {
                    bpm = *new_bpm;
                    seconds_per_measure = 60.0 * signature * 4.0 / bpm;
                    seconds_per_note = seconds_per_measure / notes_in_measure as f32;
                    scroll_speed = init_scroll_speed * (unscaled_scroll) * bpm / DEFAULT_BPM
                }
                TrackCommand::Measure(num, den) => {
                    signature = *num as f32 / *den as f32;
                    seconds_per_measure = 60.0 * signature * 4.0 / bpm;
                    seconds_per_note = seconds_per_measure / notes_in_measure as f32;
                }
                TrackCommand::Delay(t) => time += *t,
                TrackCommand::Scroll(s) => {
                    scroll_speed = init_scroll_speed * (*s) * bpm / DEFAULT_BPM;
                    unscaled_scroll = *s;
                }
                TrackCommand::GogoStart => {}
                TrackCommand::GogoEnd => {}
                TrackCommand::BarlineOff => barline_on = false,
                TrackCommand::BarlineOn => barline_on = true,
                _ => {}
            },
            NoteTrackEntry::Notes(new_notes) => {
                let num_notes = new_notes.len();

                // The notes that are to be added to the track.
                // Each note is evenly spaced (including the Nones, which represent
                // no notes). Thus, we can multiply the milliseconds per note by each note's
                // index in the vector and add this to the current time to find when the note should
                // be hit.
                let new_notes = new_notes
                    .iter()
                    .enumerate()
                    .filter_map(|(i, note)| {
                        note.map(|note_type| {
                            if matches!(
                                note_type,
                                TJANoteType::BalloonRoll | TJANoteType::SpecialRoll
                            ) {
                                balloon_count += 1;
                            };

                            (note_type, time + seconds_per_note * i as f32, scroll_speed)
                        })
                    });

                notes.extend(new_notes);
                // Update the current time. We didn't have to do this for each note
                // because they're evenly spaced.
                let elapsed_time = num_notes as f32 * seconds_per_note;
                time += elapsed_time;
            }

            NoteTrackEntry::EndMeasure => {
                if notes_in_measure == 0 {
                    // Make sure that even if we've had no notes we're still at
                    // the next measure
                    time = measure_start_time + seconds_per_measure;
                }

                measure_start_time = time;

                if barline_on {
                    barlines.push(time);
                }

                // Recalculate our measure-based variables
                notes_in_measure = notes_in_next_measure(&mut items_iter);

                seconds_per_note = if notes_in_measure == 0 {
                    0.0
                } else {
                    seconds_per_measure / notes_in_measure as f32
                };
            }
        }
    }

    let mut track_notes = Vec::with_capacity(notes.len());
    let mut notes = notes.into_iter().peekable();

    while let Some((note_type, time, scroll_speed)) = notes.next() {
        use TJANoteType::*;

        // If the next note is a drum roll, look ahead to find where it ends
        let roll_time =
            if matches!(note_type, Roll | BigRoll | BalloonRoll | SpecialRoll) {
                // Annoyingly, special rolls can be ended by other special rolls.
                // So we have to do some extra logic.
                let next_time = if note_type == SpecialRoll
                    && notes.peek().ok_or(TJAParseError::RollNotEnded)?.0
                        == SpecialRoll
                {
                    notes.peek().ok_or(TJAParseError::RollNotEnded)?.1
                } else {
                    let (next_type, next_time, _) =
                        notes.next().ok_or(TJAParseError::RollNotEnded)?;

                    if next_type != RollEnd {
                        return Err(TJAParseError::RollNotEnded);
                    }

                    next_time
                };

                Some(next_time - time)
            } else {
                None
            };

        let note_type = match note_type {
            Don => NoteType::Don,
            Kat => NoteType::Kat,
            BigDon => NoteType::BigDon,
            BigKat => NoteType::BigKat,
            Roll => NoteType::Roll(roll_time.unwrap()),
            BigRoll => NoteType::BigRoll(roll_time.unwrap()),
            BalloonRoll => NoteType::BalloonRoll(roll_time.unwrap()),
            SpecialRoll => NoteType::SpecialRoll(roll_time.unwrap()),
            CoopDon => NoteType::CoopDon,
            CoopKat => NoteType::CoopKat,
            RollEnd => return Err(TJAParseError::RollEndWithoutRoll),
        };

        track_notes.push(Note {
            note_type,
            time,
            scroll_speed,
        });
    }

    track.notes = track_notes;
    track.notes.shrink_to_fit();

    // If the number of balloons in the course is nonzero, we have to store
    // how many hits it takes to complete each one. This is the BALLOON metadata
    let balloons = metadata.get("BALLOON");
    if balloon_count != 0 {
        let mut balloons_list = match balloons {
            Some(list) => {
                terminated(separated_list0(tag(","), integer::<u16>), eof)(list)
                    .finish()?
                    .1
            }

            // Ensure that BALLOON metadata does not exist ==> balloon count is 0
            None => return Err(TJAParseError::MetadataNeeded("BALLOON")),
        };

        // Also ensure that the number of balloons in the metadata matches
        // the number of balloon notes.
        // The way this works is a little strange. If the balloon list is empty "BALLOON:", then 
        // every balloon gets a default value of 5. However, if the balloon list is non-empty 
        // *but* shorter than the number of balloons in the song, every balloon after the end of 
        // the list gets a default value of 10.
        //
        // Exactly how this happened is beyond me, but it has to work this way for compatibility
        // reasons.
        if balloons_list.is_empty() {
            balloons_list = vec![5; balloon_count];
        } else if balloons_list.len() > balloon_count {
            balloons_list.truncate(balloon_count);
        } else if balloons_list.len() < balloon_count {
            balloons_list.extend(vec![10; balloon_count - balloons_list.len()]);
        }

        track.balloons = balloons_list;
    }

    let star_level = get_parsed_metadata::<u8>(metadata, "LEVEL", None)?;
    track.barlines = barlines;

    difficulties[difficulty_level] = Some(Difficulty { star_level, track });
    Ok(())
}

pub fn parse_tja_file(input: &str) -> Result<Song, TJAParseError<String>> {
    let preprocessed_input = preprocess_tja_file(input);
    let (_, items) = tja_file(&preprocessed_input).finish()?;
    let mut metadata: HashMap<&str, &str> = HashMap::new();
    let mut difficulties = [None, None, None, None, None];

    for item in items {
        match item {
            TJAFileItem::Metadata(key, value) => {
                metadata.insert(key, value);
            }

            TJAFileItem::NoteTrack(track) => {
                construct_difficulty(&track, &metadata, &mut difficulties)?;
            }
        };
    }

    // Now get the rest of the metadata needed for the song.
    let title = get_metadata_owned(&metadata, "TITLE", None)?;
    let subtitle = get_metadata_owned(&metadata, "SUBTITLE", None).ok();
    let audio_filename = get_metadata_owned(&metadata, "WAVE", None)?;
    let demostart = get_parsed_metadata::<f32>(&metadata, "DEMOSTART", Some(0.0))?;
    let offset = get_parsed_metadata::<f32>(&metadata, "OFFSET", Some(0.0))?;
    let bpm = get_parsed_metadata::<f32>(&metadata, "BPM", Some(120.0))?;

    Ok(Song {
        title,
        subtitle,
        audio_filename,
        demostart,
        bpm,
        offset,
        difficulties,
    })
}
