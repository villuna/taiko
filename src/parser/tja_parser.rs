use nom::{bytes::complete::{take_while1, tag, is_not}, sequence::separated_pair, IResult, combinator::opt, Parser};

use crate::track::Song;

#[derive(Debug)]
pub enum TJAParseError {
    SyntaxError,
}

impl<I> From<nom::error::Error<I>> for TJAParseError {
    fn from(value: nom::error::Error<I>) -> Self {
        TJAParseError::SyntaxError
    }
}

fn parse_metadata_pair(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(
        take_while1(|c| ('A'..='Z').contains(&c)),
        tag(":"),
        opt(is_not("\r\n")).map(|value| value.unwrap_or("")),
    )(input)
}

pub fn parse_tja_file(input: &str) -> Result<(), (TJAParseError, usize)> {
    let lines = input.lines().enumerate().filter_map(|(i, line)| {
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

    for (i, line) in lines {
        println!("{i}: {:?}", parse_metadata_pair(line));
    }

    Ok(())
}
