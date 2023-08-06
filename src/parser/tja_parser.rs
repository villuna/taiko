#![allow(unused)]
use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while1},
    character::complete::satisfy,
    combinator::{eof, map_res, opt, recognize},
    error::{FromExternalError, ParseError},
    multi::{many0_count, many0},
    sequence::{pair, preceded, separated_pair, terminated},
    Finish, IResult, Parser,
};

use crate::track::{Difficulty, Song};
/// Errors that can be encountered while parsing a TJA file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TJAParseErrorKind {
    SyntaxError,
    CourseCommandError,
    InvalidMetadata,
    MultipleTracksSameDifficulty(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TJAParseError {
    kind: TJAParseErrorKind,
    line: usize,
}

impl std::fmt::Display for TJAParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            TJAParseErrorKind::SyntaxError => f.write_str("syntax error")?,
            TJAParseErrorKind::CourseCommandError => f.write_str("invalid song notation command")?,
            TJAParseErrorKind::InvalidMetadata => f.write_str("invalid song metadata")?,
            TJAParseErrorKind::MultipleTracksSameDifficulty(diff) => {
                let difficulty = match diff {
                    0 => "easy",
                    1 => "normal",
                    2 => "hard",
                    3 => "extreme",
                    4 => "extra extreme",
                    _ => panic!("difficulty is out of range 0-4"),
                };

                f.write_str("multiple courses defined for {} difficulty")?;
            }
        }

        f.write_fmt(format_args!(" (at line {})", self.line + 1))
    }
}

impl std::error::Error for TJAParseError {}

impl<I> From<nom::error::Error<I>> for TJAParseErrorKind {
    fn from(value: nom::error::Error<I>) -> Self {
        TJAParseErrorKind::SyntaxError
    }
}

impl ParseError<&str> for TJAParseErrorKind {
    fn from_error_kind(_input: &str, _kind: nom::error::ErrorKind) -> Self {
        TJAParseErrorKind::SyntaxError
    }

    fn append(_: &str, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<E> FromExternalError<&str, E> for TJAParseErrorKind {
    fn from_external_error(input: &str, kind: nom::error::ErrorKind, _e: E) -> Self {
        Self::from_error_kind(input, kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Player {
    Player1,
    Player2,
}

#[derive(Debug, Clone, PartialEq)]
enum CourseCommand<'a> {
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

impl<'a> CourseCommand<'a> {
    /// Creates a new "inner track command" (that is one that isn't START or END), from name and
    /// value
    fn inner_from_name_arg(name: &'a str, arg: Option<&'a str>) -> Result<Self, TJAParseErrorKind> {
        // TODO: parse more commands
        let arg_res = arg.ok_or(TJAParseErrorKind::CourseCommandError);

        let command = match name {
            "END" => panic!("fatal error parsing song command: end command should have been handled seperately!"),
            "LYRIC" => CourseCommand::Lyric(arg_res?),
            "BPMCHANGE" => {
                CourseCommand::BpmChange(arg_res?.parse::<f32>().map_err(|_| TJAParseErrorKind::CourseCommandError)?)
            }
            "MEASURE" => {
                let (_, (numerator, denominator)) =
                    time_signature(arg_res?).map_err(|_| TJAParseErrorKind::CourseCommandError)?;

                CourseCommand::Measure(numerator, denominator)
            }
            "DELAY" => CourseCommand::Delay(arg_res?.parse::<f32>().map_err(|_| TJAParseErrorKind::CourseCommandError)?),
            "SCROLL" => {
                CourseCommand::Scroll(arg_res?.parse::<f32>().map_err(|_| TJAParseErrorKind::CourseCommandError)?)
            }
            "GOGOSTART" | "GOGOEND" | "BARLINEOFF" | "BARLINEON" => {
                // These dont take any arguments, so ensure there is no arg
                if arg.is_some() {
                    return Err(TJAParseErrorKind::CourseCommandError);
                }

                match name {
                    "GOGOSTART" => CourseCommand::GogoStart,
                    "GOGOEND" => CourseCommand::GogoEnd,
                    "BARLINEOFF" => CourseCommand::BarlineOff,
                    "BARLINEON" => CourseCommand::BarlineOn,
                    _ => unreachable!(),
                }
            }
            _ => return Err(TJAParseErrorKind::CourseCommandError),
        };

        Ok(command)
    }
}

/// The type of a note.
///
/// This includes a special note, which defines the end
/// of a drum roll. All drum rolls should be terminated with this note
/// (with the exception of the [SpecialRoll][NoteType::SpecialRoll], which can be terminated
/// with another [SpecialRoll][NoteType::SpecialRoll]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, num_derive::FromPrimitive)]
enum TJANoteType {
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

/// Takes a parser function (as defined by the [nom] crate), and turns it into a function that
/// parses a single string completely and returns a `Result<T, TJAParseError>`. All nom errors are
/// turned into [TJAParseError::SyntaxError]
fn parse<'a, T, F, E>(parser: F) -> impl FnOnce(&'a str) -> Result<T, TJAParseErrorKind>
where
    F: FnMut(&'a str) -> IResult<&'a str, T, E>,
    E: ParseError<&'a str>,
    TJAParseErrorKind: From<E>,
{
    move |input| {
        let (_, res) = terminated(parser, eof)(input).finish()?;
        Ok(res)
    }
}

/// Parses a single digit from 0 to 9.
fn digit(i: &str) -> IResult<&str, char, TJAParseErrorKind> {
    satisfy(|c| c.is_ascii_digit())(i)
}

/// Parses an integer. Converts it into the given generic type
fn integer<T: std::str::FromStr>(i: &str) -> IResult<&str, T, TJAParseErrorKind> {
    let onenine = satisfy(|c| ('1'..'9').contains(&c));

    map_res(
        alt((
            recognize(pair(onenine, many0_count(digit))),
            recognize(digit),
        )),
        |i_str: &str| i_str.parse::<T>(),
    )(i)
}

/// Parses a time signature (integer/integer)
/// I've restricted them to 8 bit unsigned integers as I imagine that if you're writing a song that
/// uses numbers greater than 256 for the time signature well... good luck
fn time_signature(i: &str) -> IResult<&str, (u8, u8), TJAParseErrorKind> {
    separated_pair(integer::<u8>, tag("/"), integer::<u8>)(i)
}

/// Parses a metadata pair in the form `KEY:value`. The key must be made up entirely of uppercase
/// letters.
fn metadata_pair(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(
        take_while1(|c| ('A'..='Z').contains(&c)),
        tag(":"),
        opt(is_not("\r\n")).map(|value| value.unwrap_or("")),
    )(input)
}

/// Parses a beatmap start command `#START [player]`. The player argument is optional, and is
/// used when a track has different note maps for players playing on the same difficulty. It
/// can either be `"P1", "P2", or absent if there is only one track for the difficulty.
fn start_command(input: &str) -> IResult<&str, Option<Player>, TJAParseErrorKind> {
    let (input, opt_player) =
        preceded(tag("#START"), opt(preceded(tag(" "), is_not("\n\r"))))(input)?;

    let player = match opt_player {
        None => None,
        Some("P1") => Some(Player::Player1),
        Some("P2") => Some(Player::Player2),
        _ => return Err(nom::Err::Failure(TJAParseErrorKind::CourseCommandError)),
    };

    Ok((input, player))
}

/// Parses a beatmap end command `#END`.
fn end_command(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("#END")(input)?;
    Ok((input, ()))
}

// Functions for parsing courses/beatmaps

#[derive(Debug, PartialEq)]
enum CourseItem<'a> {
    Command(CourseCommand<'a>),
    Notes{ notes: Vec<Option<TJANoteType>>, end_measure: bool },
}

fn course_command(input: &str) -> IResult<&str, CourseCommand, TJAParseErrorKind> {
    let (input, (key, value)) = preceded(
        tag("#"),
        pair(
            take_while1(|c| ('A'..='Z').contains(&c)),
            opt(preceded(tag(" "), is_not("\n\r"))),
        ),
    )(input)?;

    let command = CourseCommand::inner_from_name_arg(key, value).map_err(|e| nom::Err::Error(e))?;

    Ok((input, command))
}

fn note(i: &str) -> IResult<&str, Option<TJANoteType>, TJAParseErrorKind> {
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

fn notes(input: &str) -> IResult<&str, CourseItem, TJAParseErrorKind> {
    let (mut input, notes) = many0(note)(input)?;

    let end_measure = match tag::<_, _, nom::error::Error<_>>(",")(input) {
        Ok((i, _)) => {
            input = i;
            true
        },

        Err(_) => {
            false
        }
    };

    Ok((input, CourseItem::Notes { notes, end_measure }))
}

fn course_item(input: &str) -> IResult<&str, CourseItem, TJAParseErrorKind> {
    alt((
        course_command.map(|c| CourseItem::Command(c)),
        notes,
    ))(input)
}

fn process_course<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<Difficulty, TJAParseError> {
    todo!()
}

pub fn parse_tja_file(input: &str) -> Result<Song, TJAParseError> {
    // Preprocess lines (get rid of comments, empty lines, extra space etc)
    let mut lines = input.lines().enumerate().filter_map(|(i, line)| {
        // This seems to be necessary as a lot of tja files have the utf-16 alignment character at
        // the beginning. But as far as i'm aware, are not utf-16? If there's a satisfying
        // conclusion to this problem, I would love to know it.
        let mut line = line.strip_prefix('\u{feff}').unwrap_or(line);

        // Remove comments
        if let Some(i) = line.find("//") {
            line = &line[0..i];
        }

        let line = line.trim();

        if line.is_empty() {
            None
        } else {
            Some((i, line))
        }
    });

    let mut metadata = HashMap::new();
    let mut difficulties = [None, None, None, None, None];

    while let Some((i, line)) = lines.next() {
        if let Ok((key, value)) = parse(metadata_pair)(line) {
            metadata.insert(key, (i, value));
        } else {
            match parse(start_command)(line) {
                Ok(player) => {
                    // TODO: actually deal with the player argument lol
                    if player.is_some() {
                        unimplemented!()
                    }

                    let difficulty_level = match metadata.get("COURSE") {
                        Some(&(line, course)) => match course {
                            "Easy" | "0" => 0,
                            "Normal" | "1" => 1,
                            "Hard" | "2" => 2,
                            "Oni" | "3" => 3,
                            "Edit" | "4" => 4,
                            _ => {
                                return Err(TJAParseError {
                                    kind: TJAParseErrorKind::InvalidMetadata,
                                    line,
                                })
                            }
                        },

                        // Default difficulty is oni
                        None => 3,
                    };

                    // If there is already a course for this difficulty, thats an error
                    if difficulties[difficulty_level].is_some() {
                        return Err(TJAParseError {
                            kind: TJAParseErrorKind::MultipleTracksSameDifficulty(difficulty_level),
                            line: i,
                        });
                    }

                    let difficulty = process_course(&mut lines)?;
                    difficulties[difficulty_level] = Some(difficulty);   
                },

                // The reason we return the error that the start_command function returned, is that
                // parse(metadata_pair) can only return a syntax error. So if it is a syntax error
                // for both, it will be a syntax error for just parse(start_command).
                Err(e) => return Err(TJAParseError { kind: e, line: i}),
            }
        }
    }

    Ok(Song::default())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_song_command() {
        assert_eq!(
            parse(course_command)("#BPMCHANGE 123"),
            Ok(CourseCommand::BpmChange(123.0))
        );

        assert_eq!(
            parse(course_command)("#MEASURE 9/8"),
            Ok(CourseCommand::Measure(9, 8))
        );
    }

    #[test]
    fn test_notes() {
        use TJANoteType::*;

        assert_eq!(
            parse(notes)("1201"),
            Ok(CourseItem::Notes {
                notes: vec![Some(Don), Some(Kat), None, Some(Don)],
                end_measure: false,
            })
        );

        assert_eq!(
            parse(notes)("1201,"),
            Ok(CourseItem::Notes {
                notes: vec![Some(Don), Some(Kat), None, Some(Don)],
                end_measure: true,
            })
        );
    }

    #[test]
    fn test_file() {
        parse_tja_file(include_str!("Ready to.tja")).unwrap();
        assert!(false);
    }
}
