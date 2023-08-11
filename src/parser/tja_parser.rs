use std::collections::HashMap;

use lookahead::Lookahead;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while1},
    character::complete::satisfy,
    combinator::{eof, map_res, opt, recognize},
    error::{FromExternalError, ParseError},
    multi::{many0_count, many1, separated_list0},
    sequence::{pair, preceded, separated_pair, terminated},
    Finish, IResult, Parser,
};

use crate::track::{Difficulty, Note, NoteTrack, NoteType, Song};
/// Types of errors that can be encountered while parsing a TJA file. This is used in the
/// [TJAParseError] struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TJAParseErrorKind {
    SyntaxError,
    CourseCommandError,
    InvalidMetadata,
    MultipleTracksSameDifficulty(usize),
    ExpectedEndCommand,
    MissingMetadataForCourse(String),
    MissingMetadataForSong(String),
    RollNotEnded,
    RollEndWithoutRoll,
}

/// An error that can be encountered while parsing a TJA file. Contains an enum for the kind of
/// error as well as the line where the error is (or pertains to).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TJAParseError {
    pub kind: TJAParseErrorKind,
    pub line: usize,
}

impl std::fmt::Display for TJAParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            TJAParseErrorKind::SyntaxError => f.write_str("syntax error")?,
            TJAParseErrorKind::CourseCommandError => {
                f.write_str("invalid song notation command")?
            }
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

                f.write_fmt(format_args!(
                    "multiple courses defined for {difficulty} difficulty"
                ))?;
            }
            TJAParseErrorKind::ExpectedEndCommand => f.write_str("expected #END command")?,
            TJAParseErrorKind::MissingMetadataForCourse(key) => {
                f.write_fmt(format_args!(
                    "metadata needed for this difficulty: \"{key}\""
                ))?;
            }
            TJAParseErrorKind::MissingMetadataForSong(key) => {
                f.write_fmt(format_args!("metadata needed for the song: \"{key}\""))?;
            }
            TJAParseErrorKind::RollNotEnded => f.write_str("drumroll not ended")?,
            TJAParseErrorKind::RollEndWithoutRoll => {
                f.write_str("drumroll end without preceding drumroll")?
            }
        }

        f.write_fmt(format_args!(" (at line {})", self.line + 1))
    }
}

impl std::error::Error for TJAParseError {}

impl<I> From<nom::error::Error<I>> for TJAParseErrorKind {
    fn from(_value: nom::error::Error<I>) -> Self {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TJANoteType {
    Don,
    Kat,
    BigDon,
    BigKat,
    Roll,
    BigRoll,
    BalloonRoll(u16),
    RollEnd,
    SpecialRoll(u16),
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
fn end_command(input: &str) -> IResult<&str, (), TJAParseErrorKind> {
    let (input, _) = tag("#END")(input)?;
    Ok((input, ()))
}

// Functions for parsing courses/beatmaps

#[derive(Debug, PartialEq)]
enum CourseItem<'a> {
    EndCommand,
    Command(CourseCommand<'a>),
    Notes {
        notes: Vec<Option<TJANoteType>>,
        end_measure: bool,
    },
}

fn course_command(input: &str) -> IResult<&str, CourseCommand, TJAParseErrorKind> {
    let (input, (key, value)) = preceded(
        tag("#"),
        pair(
            take_while1(|c| ('A'..='Z').contains(&c)),
            opt(preceded(tag(" "), is_not("\n\r"))),
        ),
    )(input)?;

    let command = CourseCommand::inner_from_name_arg(key, value).map_err(nom::Err::Error)?;

    Ok((input, command))
}

fn note(i: &str) -> IResult<&str, Option<TJANoteType>, TJAParseErrorKind> {
    satisfy(|c| c.is_ascii_digit() || ['A', 'B'].contains(&c))
        .map(|c| match c {
            '0' => None,
            '1' => Some(TJANoteType::Don),
            '2' => Some(TJANoteType::Kat),
            '3' => Some(TJANoteType::BigDon),
            '4' => Some(TJANoteType::BigKat),
            '5' => Some(TJANoteType::Roll),
            '6' => Some(TJANoteType::BigRoll),
            '7' => Some(TJANoteType::BalloonRoll(0)),
            '8' => Some(TJANoteType::RollEnd),
            '9' => Some(TJANoteType::SpecialRoll(0)),
            'A' => Some(TJANoteType::CoopDon),
            'B' => Some(TJANoteType::CoopKat),
            _ => unreachable!("matching on invalid note!"),
        })
        .parse(i)
}

fn notes(input: &str) -> IResult<&str, CourseItem, TJAParseErrorKind> {
    // We don't want to match nothing, so if the bar does not have an ending comma it has to have
    // at least a note in it.
    let end_tag = tag::<_, _, nom::error::Error<_>>(",");

    if let Ok((input, _)) = end_tag(input) {
        Ok((
            input,
            CourseItem::Notes {
                notes: vec![],
                end_measure: true,
            },
        ))
    } else {
        let (mut input, notes) = many1(note)(input)?;

        let end_measure = match end_tag(input) {
            Ok((i, _)) => {
                input = i;
                true
            }

            Err(_) => false,
        };

        Ok((input, CourseItem::Notes { notes, end_measure }))
    }
}

fn course_item(input: &str) -> IResult<&str, CourseItem, TJAParseErrorKind> {
    alt((
        end_command.map(|_| CourseItem::EndCommand),
        notes,
        course_command.map(CourseItem::Command),
    ))(input)
}

/// Preprocess the lines that define a course and turn them into a vector of [CourseItem]s.
/// This is necessary because we need to be able to look ahead find find some not-yet processed
/// information while constructing the beatmaps.
fn process_course<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<Vec<CourseItem<'a>>, TJAParseError> {
    // Needed for returning a line number error if we ever run out of lines
    let mut line_num = 0;
    let mut res = Vec::new();

    for (i, line) in lines {
        line_num = i;

        match parse(course_item)(line).map_err(|e| TJAParseError { kind: e, line: i })? {
            CourseItem::EndCommand => return Ok(res),
            item => res.push(item),
        }
    }

    Err(TJAParseError {
        kind: TJAParseErrorKind::ExpectedEndCommand,
        line: line_num,
    })
}

fn get_parsed_metadata<'a, T: std::str::FromStr>(
    metadata: &HashMap<&'a str, (usize, &'a str)>,
    key: &'a str,
    default: Option<T>,
    course_line: Option<usize>,
) -> Result<T, TJAParseError> {
    metadata
        .get(key)
        .map(|&(i, s)| {
            s.parse::<T>().map_err(|_| TJAParseError {
                kind: TJAParseErrorKind::InvalidMetadata,
                line: i,
            })
        })
        .unwrap_or(match default {
            Some(t) => Ok(t),
            None => {
                if let Some(course_line) = course_line {
                    Err(TJAParseError {
                        kind: TJAParseErrorKind::MissingMetadataForCourse(key.to_string()),
                        line: course_line,
                    })
                } else {
                    Err(TJAParseError {
                        kind: TJAParseErrorKind::MissingMetadataForSong(key.to_string()),
                        line: 0,
                    })
                }
            }
        })
}

fn get_metadata_owned<'a>(
    metadata: &HashMap<&'a str, (usize, &'a str)>,
    key: &'a str,
    default: Option<&str>,
    course_line: Option<usize>,
) -> Result<String, TJAParseError> {
    let value = if let Some(s) = metadata.get(key) {
        s.1.to_string()
    } else {
        match default {
            Some(s) => s.to_string(),
            None => {
                if let Some(course_line) = course_line {
                    return Err(TJAParseError {
                        kind: TJAParseErrorKind::MissingMetadataForCourse(key.to_string()),
                        line: course_line,
                    });
                } else {
                    return Err(TJAParseError {
                        kind: TJAParseErrorKind::MissingMetadataForSong(key.to_string()),
                        line: 0,
                    });
                }
            }
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
fn notes_in_next_measure<'a, I: Iterator<Item = CourseItem<'a>>>(iter: &mut Lookahead<I>) -> usize {
    let mut num_notes = 0;

    for i in 0.. {
        let Some(item) = iter.lookahead(i) else { break; };

        if let CourseItem::Notes { notes, end_measure } = item {
            num_notes += notes.len();
            if *end_measure {
                break;
            }
        }
    }

    num_notes
}

fn construct_difficulty(
    items: Vec<CourseItem<'_>>,
    metadata: &HashMap<&str, (usize, &str)>,
    course_line: usize,
) -> Result<Difficulty, TJAParseError> {
    let mut track = NoteTrack::default();

    // Various metadata needed for constructing the track
    // The time signature, as numerator divided by denominator (musicians might
    // disagree but I feel pretty good about it)
    // Defaults to common time
    let mut signature = 1f32;
    const DEFAULT_BPM: f32 = 120.0;
    let mut bpm =
        get_parsed_metadata::<f32>(metadata, "BPM", Some(DEFAULT_BPM), Some(course_line))?;
    let offset = get_parsed_metadata::<f32>(metadata, "OFFSET", Some(0.0), Some(course_line))?;
    let init_scroll_speed =
        get_parsed_metadata::<f32>(metadata, "HEADSCROLL", Some(1.0), Some(course_line))?;

    // If the number of balloons in the course is nonzero, we have to store
    // how many hits it takes to complete each one. This is the BALLOON metadata
    let balloons = metadata
        .get("BALLOON")
        .map(|&(i, list)| {
            parse(terminated(separated_list0(tag(","), integer::<u16>), eof))(list).map_err(|_| {
                TJAParseError {
                    kind: TJAParseErrorKind::InvalidMetadata,
                    line: i,
                }
            })
        })
        .transpose()?;

    let mut balloon_index = 0;

    let mut unscaled_scroll = init_scroll_speed;
    let mut scroll_speed = init_scroll_speed * bpm / DEFAULT_BPM;

    let mut items_iter = lookahead::lookahead(items);
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
            CourseItem::Command(command) => match command {
                CourseCommand::BpmChange(new_bpm) => {
                    bpm = new_bpm;
                    seconds_per_measure = 60.0 * signature * 4.0 / bpm;
                    seconds_per_note = seconds_per_measure / notes_in_measure as f32;
                    scroll_speed = init_scroll_speed * (unscaled_scroll) * bpm / DEFAULT_BPM
                }
                CourseCommand::Measure(num, den) => {
                    signature = num as f32 / den as f32;
                    seconds_per_measure = 60.0 * signature * 4.0 / bpm;
                    seconds_per_note = seconds_per_measure / notes_in_measure as f32;
                }
                CourseCommand::Delay(t) => time += t,
                CourseCommand::Scroll(s) => {
                    scroll_speed = init_scroll_speed * (s) * bpm / DEFAULT_BPM;
                    unscaled_scroll = s;
                }
                CourseCommand::GogoStart => {}
                CourseCommand::GogoEnd => {}
                CourseCommand::BarlineOff => barline_on = false,
                CourseCommand::BarlineOn => barline_on = true,
                _ => {}
            },
            CourseItem::Notes {
                notes: new_notes,
                end_measure,
            } => {
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
                            let mut note_type = note_type;

                            if matches!(
                                note_type,
                                TJANoteType::BalloonRoll(_) | TJANoteType::SpecialRoll(_)
                            ) {
                                // TJA balloon roll logic is... very strange.
                                // If there are no balloons listed, (`BALLOON:`) then every balloon
                                // roll gets a value of 5. Otherwise it gets the value listed in
                                // order, but if there are more balloons in the track than are
                                // listed in the metadata, it gets a DIFFERENT value of 10.
                                // It's very strange but it has to be this way for... compatibility
                                // reasons? I think it might have even been a bug in TJAPlayer that
                                // was unintentionally carried into the format.
                                //
                                // i'm doing great
                                if let Some(balloons) = balloons.as_ref() {
                                    let roll_num = if balloons.is_empty() {
                                        5
                                    } else {
                                        *balloons.get(balloon_index).unwrap_or(&10)
                                    };

                                    balloon_index += 1;

                                    note_type = match note_type {
                                        TJANoteType::BalloonRoll(_) => {
                                            TJANoteType::BalloonRoll(roll_num)
                                        }
                                        TJANoteType::SpecialRoll(_) => {
                                            TJANoteType::SpecialRoll(roll_num)
                                        }
                                        _ => unreachable!(),
                                    }
                                } else {
                                    // No balloons were specified in metadata but there was a
                                    // balloon note. Thats invalid!
                                    return Err(TJAParseError {
                                        kind: TJAParseErrorKind::MissingMetadataForCourse(
                                            "BALLOON".to_string(),
                                        ),
                                        line: course_line,
                                    });
                                }
                            };

                            Ok((note_type, time + seconds_per_note * i as f32, scroll_speed))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                notes.extend(new_notes);
                // Update the current time. We didn't have to do this for each note
                // because they're evenly spaced.
                let elapsed_time = num_notes as f32 * seconds_per_note;
                time += elapsed_time;

                if end_measure {
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

            CourseItem::EndCommand => {
                unreachable!("end commands should not appear in the course item queue :(")
            }
        }
    }

    let mut track_notes = Vec::with_capacity(notes.len());
    let mut notes = notes.into_iter().peekable();

    while let Some((note_type, time, scroll_speed)) = notes.next() {
        use TJANoteType::*;

        // If the next note is a drum roll, look ahead to find where it ends
        let roll_time = if matches!(note_type, Roll | BigRoll | BalloonRoll(_) | SpecialRoll(_)) {
            // Annoyingly, special rolls can be ended by other special rolls.
            // So we have to do some extra logic.
            let next = notes.peek().ok_or(TJAParseError {
                kind: TJAParseErrorKind::RollNotEnded,
                line: course_line,
            })?;

            let next_time = if matches!(note_type, SpecialRoll(_)) {
                let next_type = next.0;

                if matches!(next_type, SpecialRoll(_)) {
                    next.1
                } else if next_type == RollEnd {
                    let time = next.1;
                    let _ = next; // So that notes isn't borrowed and we can call the next line
                    notes.next();
                    time
                } else {
                    return Err(TJAParseError {
                        kind: TJAParseErrorKind::RollNotEnded,
                        line: course_line,
                    });
                }
            } else {
                let (next_type, next_time, _) = notes.next().ok_or(TJAParseError {
                    kind: TJAParseErrorKind::RollNotEnded,
                    line: course_line,
                })?;

                if next_type != RollEnd {
                    return Err(TJAParseError {
                        kind: TJAParseErrorKind::RollNotEnded,
                        line: course_line,
                    });
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
            BalloonRoll(n) => NoteType::BalloonRoll(roll_time.unwrap(), n),
            SpecialRoll(n) => NoteType::SpecialRoll(roll_time.unwrap(), n),
            CoopDon => NoteType::CoopDon,
            CoopKat => NoteType::CoopKat,
            RollEnd => {
                return Err(TJAParseError {
                    kind: TJAParseErrorKind::RollEndWithoutRoll,
                    line: course_line,
                })
            }
        };

        track_notes.push(Note {
            note_type,
            time,
            scroll_speed,
        });
    }

    track.notes = track_notes;
    track.notes.shrink_to_fit();

    let star_level = get_parsed_metadata::<u8>(metadata, "LEVEL", None, Some(course_line))?;
    track.barlines = barlines;

    Ok(Difficulty { star_level, track })
}

/// Parses a TJA file into a [Song] struct.
///
/// This doesn't check that, e.g. the song file is valid,
/// but it does require that the TJA file is. See [TJAParseErrorKind] to see the errors that
/// can be encountered while parsing.
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
    let mut difficulties: [Option<Difficulty>; 5] = [None, None, None, None, None];

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
                            line: i + 1,
                        });
                    }

                    let items = process_course(&mut lines)?;
                    let difficulty = construct_difficulty(items, &metadata, i + 1)?;
                    difficulties[difficulty_level] = Some(difficulty);
                }

                // The reason we return the error that the start_command function returned, is that
                // parse(metadata_pair) can only return a syntax error. So if it is a syntax error
                // for both, it will be a syntax error for just parse(start_command).
                Err(e) => return Err(TJAParseError { kind: e, line: i }),
            }
        }
    }

    // Now get the rest of the metadata needed for the song.
    let title = get_metadata_owned(&metadata, "TITLE", None, None)?;
    let subtitle = get_metadata_owned(&metadata, "SUBTITLE", None, None).ok();
    let audio_filename = get_metadata_owned(&metadata, "WAVE", None, None)?;
    let demostart = get_parsed_metadata::<f32>(&metadata, "DEMOSTART", Some(0.0), None)?;
    let offset = get_parsed_metadata::<f32>(&metadata, "OFFSET", Some(0.0), None)?;
    let bpm = get_parsed_metadata::<f32>(&metadata, "BPM", Some(120.0), None)?;

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
    fn test_course_item() {
        assert_eq!(
            course_item("#MEASURE 4/4"),
            Ok(("", CourseItem::Command(CourseCommand::Measure(4, 4))))
        );
    }
}
