mod gpu;

use super::*;
use crate::{
    CameraPreset,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput};

const SIZE: [u32; 2] = [800, 600];

fn driver() -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, SIZE);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
}
fn study(driver: &mut BoardDriver, unlocked: bool) {
    if unlocked {
        let training = driver.training.as_mut().unwrap();
        for lesson in training.course.lessons() {
            training
                .progress
                .completed_lessons
                .insert(lesson.id.clone());
        }
    }
    driver.training_action(MenuAction::Lesson(0)).unwrap();
    driver.training.as_mut().unwrap().panel = "References\nA fork and a pin.".into();
    driver.sync_menu_ui().unwrap();
}
fn key(driver: &mut BoardDriver, key: &str) {
    for pressed in [true, false] {
        driver
            .handle_input(
                &RetainedInput::Key {
                    key: key.into(),
                    logical: LogicalKey::Character(key.into()),
                    physical: KeyCode::Enter.into(),
                    pressed,
                    repeat: false,
                },
                None,
                None,
            )
            .unwrap();
    }
}
fn click(driver: &mut BoardDriver, point: [f64; 2]) {
    for input in [
        RetainedInput::PointerMoved(point),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ] {
        driver.handle_input(&input, None, None).unwrap();
    }
}
fn link(
    driver: &mut BoardDriver,
    word: &str,
) -> (ObjectHandle, worldrunner::ui::UiLinkId, [f64; 2]) {
    let ui = driver.ui.as_mut().unwrap();
    ui.overlay("", SIZE).unwrap();
    let (object, id) = ui
        .native
        .world
        .published_components()
        .find_map(|c| {
            let ComponentDesc::Text(run) = &c.desc else {
                return None;
            };
            run.spans.iter().find_map(|s| {
                (run.text[s.range.clone()].eq_ignore_ascii_case(word))
                    .then_some(s.link)
                    .flatten()
                    .map(|id| (c.object, id))
            })
        })
        .expect("authored reference must have an engine link span");
    let quad = ui
        .plan
        .as_ref()
        .unwrap()
        .texts
        .iter()
        .find(|t| t.object == object)
        .unwrap()
        .quads
        .iter()
        .find(|q| q.link == Some(id))
        .expect("visible linked glyph");
    let point = [
        f64::from(quad.corners[0][0].midpoint(quad.corners[2][0])),
        f64::from(SIZE[1]) - f64::from(quad.corners[0][1].midpoint(quad.corners[2][1])),
    ];
    (object, id, point)
}

#[test]
fn term_pointer_opens_the_lesson_that_defines_it() {
    let mut driver = driver();
    study(&mut driver, true);
    let (_, _, point) = link(&mut driver, "fork");
    click(&mut driver, point);
    assert_eq!(driver.screen(), Screen::Lesson);
    assert!(driver.panel_text().starts_with("Attack two targets"));
}

#[test]
fn locked_term_pointer_keeps_the_lesson_and_shows_its_prerequisites() {
    let mut driver = driver();
    study(&mut driver, false);
    let board = driver.board().unwrap().game().clone();
    let (_, _, point) = link(&mut driver, "fork");
    click(&mut driver, point);
    assert_eq!(driver.screen(), Screen::Lesson);
    assert_eq!(driver.board().unwrap().game(), &board);
    assert!(
        driver
            .panel_text()
            .contains("Attack two targets (tactics.fork) is locked.")
    );
    assert!(driver.panel_text().contains("Complete"));
}

#[test]
fn tab_traverses_individual_links_and_enter_activates_the_second() {
    let mut driver = driver();
    study(&mut driver, true);
    let (object, first, _) = link(&mut driver, "fork");
    let (_, second, _) = link(&mut driver, "pin");
    for _ in 0..20 {
        let router = &driver.ui.as_ref().unwrap().native.router;
        if router.focused() == Some(object) && router.focused_link() == Some((object, first)) {
            break;
        }
        key(&mut driver, "tab");
    }
    assert_eq!(
        driver.ui.as_ref().unwrap().native.router.focused_link(),
        Some((object, first))
    );
    key(&mut driver, "tab");
    assert_eq!(
        driver.ui.as_ref().unwrap().native.router.focused_link(),
        Some((object, second))
    );
    key(&mut driver, "enter");
    assert!(
        driver
            .panel_text()
            .starts_with("Use a piece that cannot move")
    );
}

#[test]
fn review_san_pointer_jumps_to_the_recorded_position_after_that_ply() {
    let mut driver = driver();
    driver.play_uci("e2e4").unwrap();
    let after = driver.board().unwrap().game().clone();
    driver.play_uci("e7e5").unwrap();
    driver.finish("Draw by agreement".into()).unwrap();
    driver
        .open_review(crate::review::ReviewConfig::default())
        .unwrap();
    let (_, _, point) = link(&mut driver, "e4");
    click(&mut driver, point);
    assert_eq!(driver.review_cursor(), Some(1));
    assert_eq!(driver.board().unwrap().game(), &after);
}

#[test]
fn explicit_lesson_reference_uses_the_course_identity() {
    let mut driver = driver();
    study(&mut driver, true);
    driver.training.as_mut().unwrap().panel = "Reference\nSee lesson tactics.skewer.".into();
    driver.sync_menu_ui().unwrap();
    let (_, _, point) = link(&mut driver, "tactics.skewer");
    click(&mut driver, point);
    assert!(
        driver
            .panel_text()
            .starts_with("Force the front piece away")
    );
}
