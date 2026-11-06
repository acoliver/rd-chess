use super::*;

#[test]
fn longest_course_reference_wins_without_matching_inside_words() {
    let entries = [
        ("pin".into(), MenuAction::LinkedLesson(1)),
        ("check".into(), MenuAction::LinkedLesson(2)),
        ("discovered check".into(), MenuAction::LinkedLesson(3)),
    ];
    let text = "Pin heading\nspinning; a discovered check, then a pin.";
    let result = matches(text, 12, &entries.iter().collect::<Vec<_>>());
    assert_eq!(result.len(), 2);
    assert_eq!(&text[result[0].0.clone()], "discovered check");
    assert_eq!(result[0].1, MenuAction::LinkedLesson(3));
    assert_eq!(&text[result[1].0.clone()], "pin");
}

#[test]
fn repeated_san_references_keep_their_order_and_check_suffixes() {
    let entries = [
        ("Nf3".into(), MenuAction::ReviewText(0)),
        ("Nf3".into(), MenuAction::ReviewText(4)),
        ("Qh7#".into(), MenuAction::ReviewText(6)),
    ];
    let text = "Nf3, Nf3; Qh7#; Nf3+ is a different move.";
    let result = matches(text, 0, &entries.iter().collect::<Vec<_>>());
    assert_eq!(
        result.iter().map(|(_, action)| *action).collect::<Vec<_>>(),
        [
            MenuAction::ReviewText(0),
            MenuAction::ReviewText(4),
            MenuAction::ReviewText(6),
        ]
    );
}

#[test]
fn course_glossary_rejects_duplicate_and_padded_aliases() {
    let source = include_str!("../../../../assets/lessons/first-course.json");
    for terms in ["[\"fork\", \"FORK\"]", "[\" fork\"]"] {
        let invalid = source.replace("[\"fork\"]", terms);
        assert!(Course::from_json(&invalid).is_err());
    }
}
