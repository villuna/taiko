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
//! embeddedExpression ::= "string with embedded expression: {1 + 2 * i}"
//! ```

use std::collections::HashMap;

use crate::track::NoteType;
use crate::track::Song;
use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::character::complete::{crlf, newline};
use nom::combinator::opt;
use nom::combinator::{eof, map_res};
use nom::error::ParseError;
use nom::multi::many0;
use nom::multi::many0_count;
use nom::sequence::delimited;
use nom::sequence::preceded;
use nom::sequence::{pair, separated_pair, terminated};
use nom::Finish;
use nom::Parser;
use nom::{character::complete::satisfy, combinator::recognize, IResult};

/// An enum that represents a single unit of information in a TJA file.
/// This is either a metadata tag, or an entire specification of the notes in
/// a difficulty. A TJA file is nothing more than a list of these items
/// separated by lines.
#[derive(Debug, Clone, PartialEq)]
enum TJAFileItem<'a> {
    Metadata(&'a str, &'a str),
    NoteTrack(Vec<NoteTrackEntry<'a>>),
}

#[derive(Debug, Clone, PartialEq)]
enum NoteTrackEntry<'a> {
    Command(TrackCommand<'a>),
    Notes(Vec<Option<NoteType>>),
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
pub enum TJAParseError<'a> {
    NomError {
        input: &'a str,
        kind: nom::error::ErrorKind,
    },
    TrackCommandError,
    UnexpectedEndCommand,
    InvalidNote(char),
    NoteTrackNotEnded,
    MultipleTracksSameDifficulty(usize),
    InvalidMetadata(&'a str, &'a str),
}

impl<'a> TJAParseError<'a> {
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

impl<'a> ParseError<&'a str> for TJAParseError<'a> {
    fn from_error_kind(input: &'a str, kind: nom::error::ErrorKind) -> Self {
        TJAParseError::NomError { input, kind }
    }

    fn append(_: &str, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<'a> TrackCommand<'a> {
    /// Creates a new "inner track command" (that is one that isn't START or END), from name and
    /// value
    fn inner_from_name_arg(name: &'a str, arg: Option<&'a str>) -> Result<Self, TJAParseError<'a>> {
        println!(r#"Command "{name}" "{arg:?}""#);
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
fn uppercase(i: &str) -> IResult<&str, char, TJAParseError> {
    satisfy(|c| c.is_ascii_uppercase())(i)
}

/// Parses a single digit from 0 to 9.
fn digit(i: &str) -> IResult<&str, char, TJAParseError> {
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
fn metadata_tagname(i: &str) -> IResult<&str, &str, TJAParseError> {
    recognize(pair(uppercase, many0_count(alt((uppercase, digit)))))(i)
}

// Parses a colon and an optional space, and throws away the result
//
// colon_separator ::= ":" [" "]
fn colon_separator(i: &str) -> IResult<&str, (), TJAParseError> {
    toss(pair(tag(":"), opt(tag(" "))))(i)
}

// Parses a single metadata definition: a line containing a name for the attribute, followed by a
// colon and optional space, then the value.
//
// metadata_pair ::= metadata_tagname colon_separator /[^\n\r]*/
fn metadata_pair(i: &str) -> IResult<&str, (&str, &str), TJAParseError> {
    line_of(separated_pair(
        metadata_tagname,
        colon_separator,
        is_not("\n\r"),
    ))(i)
}

// --- Parsing functions for tracks ---
fn parse_time_signature(i: &str) -> IResult<&str, (u8, u8)> {
    terminated(
        separated_pair(
            map_res(is_not("/"), |x: &str| x.parse::<u8>()),
            tag("/"),
            map_res(is_not("/"), |x: &str| x.parse::<u8>()),
        ),
        eof,
    )(i)
}

fn start_command(i: &str) -> IResult<&str, TrackCommand, TJAParseError> {
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

fn end_command(i: &str) -> IResult<&str, (), TJAParseError> {
    toss(line_of(tag("#END")))(i)
}

// Parses a track command (that isn't #START or #END), converting the result into a [TrackCommand] enum type.
//
// See [track_command_raw] for more details.
fn inner_track_command(i: &str) -> IResult<&str, TrackCommand, TJAParseError> {
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
fn track_command_raw(i: &str) -> IResult<&str, (&str, Option<&str>), TJAParseError> {
    line_of(preceded(
        tag("#"),
        pair(metadata_tagname, opt(preceded(tag(" "), is_not("\n\r")))),
    ))(i)
}

fn note(i: &str) -> IResult<&str, Option<NoteType>, TJAParseError> {
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

fn notes(i: &str) -> IResult<&str, Vec<Option<NoteType>>, TJAParseError> {
    many0(note)(i)
}

fn note_track_inner(mut i: &str) -> IResult<&str, Vec<NoteTrackEntry>, TJAParseError> {
    let mut res = Vec::new();

    while eof::<_, nom::error::Error<_>>(i).is_err() {
        if end_command(i).is_ok() {
            return Ok((i, res));
        }

        let (new_i, entry) = alt((
            inner_track_command.map(NoteTrackEntry::Command),
            pair(tag(","), alt((tag("\n"), tag("\r\n")))).map(|_| NoteTrackEntry::EndMeasure),
            notes.map(NoteTrackEntry::Notes),
        ))(i)?;

        res.push(entry);
        i = new_i;
    }

    Err(nom::Err::Failure(TJAParseError::NoteTrackNotEnded))
}

fn note_track(i: &str) -> IResult<&str, Vec<NoteTrackEntry>, TJAParseError> {
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

fn tja_item(i: &str) -> IResult<&str, TJAFileItem, TJAParseError> {
    alt((
        metadata_pair.map(|(tag, value)| TJAFileItem::Metadata(tag, value)),
        note_track.map(TJAFileItem::NoteTrack),
    ))(i)
}

fn tja_file(i: &str) -> IResult<&str, Vec<TJAFileItem>, TJAParseError> {
    terminated(many0(tja_item), eof)(i)
}

pub fn parse_tja_file(input: &str) -> Result<Song, TJAParseError> {
    let (_, items) = tja_file(input).finish()?;
    let mut song = Song::default();
    let mut metadata: HashMap<&str, &str> = HashMap::new();

    for item in items {
        match item {
            TJAFileItem::Metadata(key, value) => metadata.insert(key, value),
            TJAFileItem::NoteTrack(track) => {
                // Start by seeing what metadata is currently set.
                // Since course metadata and song metadata can be freely mixed, we have to
                // check the metadata every time before making a new track.

                // See what difficulty the track should be.
                let difficulty = match metadata.get("COURSE") {
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
                if song.difficulties[difficulty].is_some() {
                    return Err(TJAParseError::MultipleTracksSameDifficulty(difficulty));
                }

                todo!()
            }
        };
    }

    Ok(song)
}

mod test {
    #[allow(unused)]
    use super::*;

    #[test]
    fn test_meta_tag() {
        assert_eq!(
            metadata_tagname("TITLE:さいたま2000"),
            Ok((":さいたま2000", "TITLE"))
        );
        assert_eq!(
            metadata_tagname("EXAM1:something"),
            Ok((":something", "EXAM1"))
        );
    }

    #[test]
    fn test_meta_pair() {
        assert_eq!(
            metadata_pair("TITLE:さいたま2000\n"),
            Ok(("", ("TITLE", "さいたま2000")))
        );
        assert_eq!(
            metadata_pair("TITLE:POP TEAM EPIC\r\n"),
            Ok(("", ("TITLE", "POP TEAM EPIC")))
        );
        assert_eq!(
            metadata_pair("EXAM1:something"),
            Ok(("", ("EXAM1", "something")))
        );
    }

    #[test]
    fn test_end_command() {
        assert!(end_command("\n#END\n").is_ok());
        assert!(end_command("\n#END P1").is_err());
    }

    #[test]
    fn test_start_command() {
        assert_eq!(
            start_command("\n#START P2\nsomethingsomething"),
            Ok((
                "somethingsomething",
                TrackCommand::Start {
                    player: Some(Player::Player2)
                }
            ))
        );

        assert_eq!(
            start_command("\n#START P1\nsomethingsomething"),
            Ok((
                "somethingsomething",
                TrackCommand::Start {
                    player: Some(Player::Player1)
                }
            ))
        );

        assert_eq!(
            start_command("#START"),
            Ok(("", TrackCommand::Start { player: None }))
        );

        assert!(&[
            start_command("#START "),
            start_command("#END"),
            start_command("#START P3")
        ]
        .iter()
        .all(Result::is_err))
    }

    #[test]
    fn test_track_command() {
        assert_eq!(
            inner_track_command("#GOGOSTART"),
            Ok(("", TrackCommand::GogoStart))
        );
        assert!(inner_track_command("#GOGOSTART testvalue").is_err());
    }

    #[test]
    fn test_notes() {
        use NoteType::*;

        assert_eq!(
            notes("10201120,\n"),
            Ok((
                ",\n",
                vec![
                    Some(Don),
                    None,
                    Some(Kat),
                    None,
                    Some(Don),
                    Some(Don),
                    Some(Kat),
                    None
                ]
            ))
        );
    }

    #[test]
    fn test_note_track() {
        use NoteType::*;
        let track = "#START
1100,
1100,
2,
,
#END";

        assert_eq!(
            note_track(track),
            Ok((
                "",
                vec![
                    NoteTrackEntry::Command(TrackCommand::Start { player: None }),
                    NoteTrackEntry::Notes(vec![Some(Don), Some(Don), None, None]),
                    NoteTrackEntry::EndMeasure,
                    NoteTrackEntry::Notes(vec![Some(Don), Some(Don), None, None]),
                    NoteTrackEntry::EndMeasure,
                    NoteTrackEntry::Notes(vec![Some(Kat)]),
                    NoteTrackEntry::EndMeasure,
                    NoteTrackEntry::EndMeasure,
                ]
            ))
        )
    }

    #[test]
    pub fn test_tja_file_item_list() {
        use NoteTrackEntry::*;
        use NoteType::*;
        use TJAFileItem::*;

        let track = "TITLE: POP TEAM EPIC
BPM:142

WAVE:POP TEAM EPIC.ogg


#START

#GOGOSTART

1100,
1100,
2,
,

#END
";

        assert_eq!(
            tja_file(track),
            Ok((
                "",
                vec![
                    Metadata("TITLE", "POP TEAM EPIC"),
                    Metadata("BPM", "142"),
                    Metadata("WAVE", "POP TEAM EPIC.ogg"),
                    NoteTrack(vec![
                        Command(TrackCommand::Start { player: None }),
                        Command(TrackCommand::GogoStart),
                        Notes(vec![Some(Don), Some(Don), None, None]),
                        EndMeasure,
                        Notes(vec![Some(Don), Some(Don), None, None]),
                        EndMeasure,
                        Notes(vec![Some(Kat)]),
                        EndMeasure,
                        EndMeasure,
                    ])
                ]
            ))
        );

        let error = "TITLE: POP TEAM EPIC
BPM:142

WAVE:POP TEAM EPIC.ogg


#START

#GOGOSTART oops this value shouldnt exist

1100,
1100,
2,
,

#END
";
        assert!(tja_file(error).is_err());
    }
}
