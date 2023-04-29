use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::character::complete::{char, line_ending, space0, newline, crlf};
use nom::error::ParseError;
use nom::multi::{many0_count, many0};
use nom::sequence::{pair, separated_pair, terminated};
use nom::{character::complete::satisfy, combinator::recognize, IResult};
use nom::combinator::{opt, eof};
use nom::sequence::delimited;

/// An enum that represents a single unit of information in a TJA file.
/// This is either a metadata tag, or an entire specification of the notes in
/// a difficulty. A TJA file is nothing more than a list of these items
/// separated by lines.
#[derive(Debug, Clone, PartialEq)]
enum TJAFileItem<'a> {
    Metadata(&'a str, &'a str),
    Beatmap, // TODO
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


// --- Parsing for the file as a whole ---

fn tja_item(i: &str) -> IResult<&str, TJAFileItem> {
    alt((
        metadata_pair.map(|(tag, value)| TJAFileItem::Metadata(tag, value)),
    ))(i)
}

fn tja_file(i: &str) -> IResult<&str, Vec<TJAFileItem>> {
    many0(tja_item)(i)
}

mod test {
    #[allow(unused)]
    use super::{metadata_pair, metadata_tagname, tja_file, TJAFileItem};

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
    fn test_tja_file() {
        let file = "
TITLE:POP TEAM EPIC

TITLEJA:POP TEAM EPIC
SUBTITLE:--Sumire Uesaka/Pop Team Epic";

        assert_eq!(
            tja_file(file),
            Ok(("", vec![
                TJAFileItem::Metadata("TITLE", "POP TEAM EPIC"),
                TJAFileItem::Metadata("TITLEJA", "POP TEAM EPIC"),
                TJAFileItem::Metadata("SUBTITLE", "--Sumire Uesaka/Pop Team Epic"),
            ]))
        )
    }
}
