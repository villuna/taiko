#![allow(unused)]
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::character::complete::{char, space0, newline, crlf};
use nom::error::ParseError;
use nom::multi::many0_count;
use nom::sequence::{pair, separated_pair, terminated};
use nom::{character::complete::satisfy, combinator::recognize, IResult};
use nom::combinator::eof;
use nom::bytes::complete::tag;
use nom::sequence::delimited;
use crate::track::NoteType;

/// An enum that represents a single unit of information in a TJA file.
/// This is either a metadata tag, or an entire specification of the notes in
/// a difficulty. A TJA file is nothing more than a list of these items
/// separated by lines.
#[derive(Debug, Clone, PartialEq)]
enum TJAFileItem<'a> {
    Metadata(&'a str, &'a str),
    Beatmap, // TODO
}

#[derive(Debug, Clone, PartialEq)]
enum BeatTrackEntry<'a> {
    Command(TrackCommand<'a>),
    Notes(Vec<NoteType>),
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
    End,
    Lyric(&'a str),
    BpmChange(f32),

    // I want to keep this parser as open as possible
    // So as long as a command follows the syntax, it will just be ignored. 
    Unknown, 
}

// --- Parsing helper functions ---

/// Parses a single uppercase letter.
fn uppercase(i: &str) -> IResult<&str, char> {
    satisfy(|c| ('A'..='Z').contains(&c))(i)
}

/// Parses a single digit from 0 to 9.
fn digit(i: &str) -> IResult<&str, char> {
    satisfy(|c| ('0'..='9').contains(&c))(i)
}

/// Parses an empty line, i.e. whitespace terminated by a newline or EOF
fn empty_line<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&str, (), E> {
    terminated(many0_count(satisfy(|c| c != '\n' && c.is_whitespace())), newline)
        .map(|_| ())
        .parse(i) 
}

/// Takes a parser that returns some value and turns it into a parser that
/// returns `()`. Useful for when you have multiple parsers whose results
/// you don't care about but you still need them to have the same type
/// signature.
fn toss<'a, F, O, E: ParseError<&'a str>>(
    mut inner: F
) -> impl FnMut(&'a str) -> IResult<&'a str, (), E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    move |i| {
        let (next, _) = inner(i)?;

        Ok((next, ()))
    }
}

/// Matches an object on its own line, surrounded by any number of
/// empty lines.
fn line_of<'a, F, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    let newline_or_eof = alt((
        toss(crlf),
        toss(newline),
        toss(eof),
    ));
    delimited(
        many0_count(empty_line),
        terminated(inner, newline_or_eof),
        many0_count(empty_line)
    )
}

// --- Parsing functions for metadata ---

// Parses a metadata tag, which will be an identifier that consists only of
// uppercase letters and numbers. It must start with an uppercase letter.
fn metadata_tagname(i: &str) -> IResult<&str, &str> {
    recognize(pair(uppercase, many0_count(alt((uppercase, digit)))))(i)
}

fn metadata_pair(i: &str) -> IResult<&str, (&str, &str)> {
    line_of(separated_pair(
        metadata_tagname,
        pair(char(':'), space0),
        is_not("\n\r"),
    ))(i)
}

// --- Parsing functions for tracks ---
fn beat_command(name: &'static str) -> impl Fn(&str) -> IResult<&str, BeatTrackEntry> {
    move |i| {
        todo!()
    }
}

fn beat_track_inner(i: &str) -> IResult<&str, TJAFileItem> {
    todo!()
}

fn beat_track(i: &str) -> IResult<&str, TJAFileItem> {
    delimited(
        beat_command("START"),
        beat_track_inner,
        beat_command("END"),
    )(i)
}

// --- Parsing for the file as a whole ---

fn tja_item(i: &str) -> IResult<&str, TJAFileItem> {
    alt((
        metadata_pair.map(|(tag, value)| TJAFileItem::Metadata(tag, value)),
    ))(i)
}

mod test {
    #[allow(unused)]
    use super::{metadata_pair, metadata_tagname};

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
}
