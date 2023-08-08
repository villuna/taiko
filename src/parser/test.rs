#[allow(unused)]
use super::*;

#[test]
fn test_tja_file_full() {
    let ok_track = "TITLE: POP TEAM EPIC
BPM:142
WAVE:POP TEAM EPIC.ogg


BALLOON:10,20
COURSE:Easy
LEVEL:1

#START

1100,
1100,
2,
7008,
,
9008,

#END
";

    assert!(parse_tja_file(ok_track).is_ok());

    let no_title = format!("//{}", ok_track);
    assert_eq!(
        parse_tja_file(&no_title).unwrap_err(),
        TJAParseError {
            kind: TJAParseErrorKind::MissingMetadataForSong("TITLE".to_string()),
            line: 0,
        }
    );
}

#[test]
fn test_real_tja_file_succeeds() {
    let ready_to = include_str!("./Ready to.tja");

    let res = parse_tja_file(&ready_to);

    println!("{:?}", res);
    assert!(res.is_ok());
}
