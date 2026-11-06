use super::*;
use crate::teaching::{Annotations, Course};

#[test]
fn authored_colours_are_not_replaced_by_the_hint_palette() {
    let mut driver = fixture();
    let mut json: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/lessons/first-course.json")).unwrap();
    json["lessons"][0]["steps"][0]["annotations"] = serde_json::json!({
        "squares": ["d4"], "square_colour": [0.2, 0.4, 0.6, 1.0],
        "arrows": [{"from": "a1", "to": "h8", "colour": [0.9, 0.3, 0.1, 1.0]}]
    });
    driver.training.as_mut().unwrap().course = Course::from_json(&json.to_string()).unwrap();
    action(&mut driver, MenuAction::Lesson(0));
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), 2);
    let AnnotationPrimitive::Circle { centre, colour, .. } = annotations[0].primitive else {
        panic!("ring");
    };
    assert_eq!(centre, point(3, 3));
    assert_eq!(
        colour.map(f32::to_bits),
        [0.2, 0.4, 0.6, 1.0].map(f32::to_bits)
    );
    arrow(
        &annotations[1],
        point(0, 0),
        point(7, 7),
        [0.9, 0.3, 0.1, 1.0],
    );
}
#[test]
fn authored_puzzle_annotations_wait_for_setup_and_leave_with_the_screen() {
    let mut driver = fixture();
    let mut puzzle = driver.training.as_ref().unwrap().puzzles.puzzles()[0].clone();
    puzzle.annotations = serde_json::from_value::<Annotations>(serde_json::json!({
        "squares": ["e7"], "square_colour": [0.2, 0.4, 0.6, 1.0],
        "arrows": [{"from": "e6", "to": "e7", "colour": [0.9, 0.3, 0.1, 1.0]}]
    }))
    .unwrap();
    driver.begin_puzzle(puzzle).unwrap();
    assert!(driver.board_annotations().is_empty());
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    driver.advance_training(std::time::Duration::ZERO).unwrap();
    driver.pending.borrow_mut().clear();
    driver
        .advance_training(std::time::Duration::from_secs(1))
        .unwrap();
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), 2);
    arrow(
        &annotations[1],
        point(4, 5),
        point(4, 6),
        [0.9, 0.3, 0.1, 1.0],
    );
    driver.show_title().unwrap();
    assert!(driver.board_annotations().is_empty());
}
#[test]
fn invalid_authored_colours_fail_data_loading() {
    for colour in [
        serde_json::json!([-0.1, 0., 0., 1.]),
        serde_json::json!([0., 2., 0., 1.]),
    ] {
        let mut json: serde_json::Value =
            serde_json::from_str(include_str!("../../../../assets/lessons/first-course.json"))
                .unwrap();
        json["lessons"][0]["steps"][0]["annotations"]["square_colour"] = colour;
        assert!(
            Course::from_json(&json.to_string())
                .unwrap_err()
                .to_string()
                .contains("annotation colour")
        );
    }
}
