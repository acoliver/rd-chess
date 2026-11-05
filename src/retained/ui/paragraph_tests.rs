#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::*;
use worldrunner::ui::TextRun;

fn state(screen: Screen, review: ReviewUiState) -> UiState {
    UiState {
        screen,
        promotion: false,
        claim: false,
        undo: false,
        continuation: None,
        settings: super::super::persistence::GameSettings::default(),
        labels: std::array::from_fn(|_| "Empty".into()),
        setup: (0, 1),
        pgn_entries: Vec::new(),
        review,
    }
}

fn run_containing(ui: &GameUi, text: &str) -> (ObjectHandle, TextRun) {
    ui.native
        .world
        .published_components()
        .find_map(|component| {
            let ComponentDesc::Text(run) = &component.desc else {
                return None;
            };
            run.text
                .contains(text)
                .then(|| (component.object, run.clone()))
        })
        .expect("paragraph text")
}

fn assert_bounded(ui: &GameUi, object: ObjectHandle, run: &TextRun) {
    let plan = ui.plan.as_ref().unwrap();
    let rect = plan.layout.rect(object).unwrap();
    let layout = ui.native.fonts.layout(run, rect.size).unwrap();
    assert!(layout.content_height <= rect.size.y);
    for line in &layout.lines {
        assert!(line.width <= rect.size.x);
        assert!(line.baseline >= 0. && line.baseline <= rect.size.y);
    }
    let contribution = plan.texts.iter().find(|t| t.object == object).unwrap();
    let corners = rect.screen_corners.unwrap();
    let min = corners.into_iter().fold(Vec2::INFINITY, Vec2::min);
    let max = corners.into_iter().fold(Vec2::NEG_INFINITY, Vec2::max);
    for quad in &contribution.quads {
        for corner in quad.corners {
            assert!(corner[0] >= min.x && corner[0] <= max.x);
            assert!(corner[1] >= min.y && corner[1] <= max.y);
        }
    }
    if let Some([min, max]) = rect.clip_bounds {
        let [x, y, width, height] = contribution.clip_rect.map(f64::from);
        assert!(x >= f64::from(min.x) && x + width <= f64::from(max.x));
        assert!(y >= f64::from(min.y) && y + height <= f64::from(max.y));
    }
}

fn assert_over_background(ui: &GameUi, object: ObjectHandle) {
    let plan = ui.plan.as_ref().unwrap();
    let background = &plan.graphics[0].graphic.quads[0];
    let min = background
        .corners
        .into_iter()
        .map(|p| Vec2::new(p[0], p[1]))
        .fold(Vec2::INFINITY, Vec2::min);
    let max = background
        .corners
        .into_iter()
        .map(|p| Vec2::new(p[0], p[1]))
        .fold(Vec2::NEG_INFINITY, Vec2::max);
    let text = plan
        .texts
        .iter()
        .find(|text| text.object == object)
        .unwrap();
    for corner in text.quads.iter().flat_map(|quad| quad.corners) {
        assert!((min.x..=max.x).contains(&corner[0]));
        assert!((min.y..=max.y).contains(&corner[1]));
    }
}

#[test]
fn lesson_step_uses_ordered_heading_and_plain_paragraph_runs_at_two_sizes() {
    let course = crate::teaching::Course::bundled().unwrap();
    let lesson = &course.lessons()[0];
    let crate::teaching::Step::Explain { text, .. } = &lesson.steps[0] else {
        panic!("explanation")
    };
    let heading = format!("{} (1/{})", lesson.title, lesson.steps.len());
    for viewport in [[800, 600], [1200, 900]] {
        let mut ui = GameUi::new().unwrap();
        let study = StudyState {
            lessons: Vec::new(),
            bands: std::array::from_fn(|_| String::new()),
            panel: format!("{heading}\n{text}"),
            emphasis: Vec::new(),
            controls: [false, true, false, false, true],
            why: false,
        };
        ui.synchronize_study(Some(study), Screen::Lesson).unwrap();
        ui.synchronize(state(Screen::Lesson, ReviewUiState::default()), viewport)
            .unwrap();
        ui.overlay("", viewport).unwrap();
        let (object, run) = run_containing(&ui, &heading);
        assert_eq!(run.text, format!("{heading}\n{text}"));
        assert_eq!(run.spans.len(), 1, "heading must be a styled run");
        assert_eq!(&run.text[run.spans[0].range.clone()], heading);
        assert_eq!(
            run.spans[0].color.map(f32::to_bits),
            [1_f32, 0.78, 0.35, 1.].map(f32::to_bits)
        );
        assert!(run.paragraph_spacing > 0.);
        assert_bounded(&ui, object, &run);
        assert_over_background(&ui, object);
    }
}

#[test]
fn review_moment_preserves_authored_breaks_and_emphasises_san_in_order() {
    let text = "Move 1: blunder. Qd4 leaves the queen hanging.\nThe reply Rxd4 wins the queen.";
    for viewport in [[800, 600], [1200, 900]] {
        let mut ui = GameUi::new().unwrap();
        let review = ReviewUiState {
            moves: vec!["1. Qd4".into(), "2. Rxd4".into()],
            moments: vec![text.into()],
            emphasis: vec!["Qd4".into(), "Rxd4".into()],
            message: "Qd4 leaves the queen hanging.\nThe reply Rxd4 wins the queen.".into(),
            ..ReviewUiState::default()
        };
        ui.synchronize(state(Screen::Review, review), viewport)
            .unwrap();
        ui.overlay("", viewport).unwrap();
        let (object, run) = run_containing(&ui, "Move 1: blunder.");
        assert_eq!(
            run.text, text,
            "engine wrapping must not rewrite source text"
        );
        let styled: Vec<_> = run
            .spans
            .iter()
            .map(|s| (&run.text[s.range.clone()], s.color))
            .collect();
        assert_eq!(
            styled,
            [
                ("Move 1: blunder.", [1., 0.78, 0.35, 1.]),
                ("Qd4", [0.45, 0.9, 1., 1.]),
                ("Rxd4", [0.45, 0.9, 1., 1.]),
            ]
        );
        assert_bounded(&ui, object, &run);
    }
}

#[test]
fn fixed_panel_marks_overflow_and_recovers_complete_source_after_resize() {
    let text = format!(
        "Lesson heading\n{}",
        "e4 controls the center and frees the bishop. ".repeat(9)
    );
    let mut ui = GameUi::new().unwrap();
    let study = StudyState {
        lessons: Vec::new(),
        bands: std::array::from_fn(|_| String::new()),
        panel: text.clone(),
        emphasis: vec!["e4".into()],
        controls: [false, true, true, true, true],
        why: false,
    };
    ui.synchronize_study(Some(study), Screen::Lesson).unwrap();
    ui.synchronize(state(Screen::Lesson, ReviewUiState::default()), [800, 600])
        .unwrap();
    for (viewport, truncated) in [([800, 600], true), ([1200, 900], false), ([800, 600], true)] {
        ui.overlay("", viewport).unwrap();
        let (object, run) = run_containing(&ui, "Lesson heading");
        if truncated {
            assert!(run.text.ends_with("..."));
            assert!(text.starts_with(run.text.trim_end_matches('.')));
            assert!(run.text.len() < text.len());
        } else {
            assert_eq!(run.text, text);
        }
        assert!(
            run.spans
                .iter()
                .all(|span| span.range.end <= run.text.len())
        );
        assert_bounded(&ui, object, &run);
    }
}

#[test]
fn review_details_keep_blank_paragraphs_and_scroll_to_the_last_line() {
    let text = format!(
        "Played Qd4.\n\n{}\nLast reply Rxd4.",
        "Consider the alternative. ".repeat(20)
    );
    for viewport in [[800, 600], [1200, 900]] {
        let mut ui = GameUi::new().unwrap();
        let review = ReviewUiState {
            message: text.clone(),
            emphasis: vec!["Qd4".into(), "Rxd4".into()],
            ..ReviewUiState::default()
        };
        ui.synchronize(state(Screen::Review, review), viewport)
            .unwrap();
        ui.overlay("", viewport).unwrap();
        let (object, run) = run_containing(&ui, "Played Qd4.");
        assert_eq!(run.text, text);
        assert_eq!(
            run.spans
                .iter()
                .map(|s| &run.text[s.range.clone()])
                .collect::<Vec<_>>(),
            ["Qd4", "Rxd4"]
        );
        assert_bounded(&ui, object, &run);
        let content = ui.native.world.parent(object).unwrap().unwrap();
        let view = *ui
            .native
            .router
            .layout
            .scrolls
            .iter()
            .find(|(_, scroll)| scroll.content == content)
            .unwrap()
            .0;
        let plan = ui.plan.as_ref().unwrap();
        ui.native
            .router
            .layout
            .scroll_to(&plan.layout, view, Vec2::new(0., 16_384.))
            .unwrap();
        assert!(ui.native.router.layout.scrolls[&view].offset().y > 0.);
        ui.dirty = true;
        ui.overlay("", viewport).unwrap();
        let plan = ui.plan.as_ref().unwrap();
        let contribution = plan.texts.iter().find(|t| t.object == object).unwrap();
        let [_, y, _, height] = contribution.clip_rect.map(f64::from);
        let glyph = contribution.quads.last().unwrap();
        assert!(f64::from(glyph.corners[0][1]) >= y);
        assert!(f64::from(glyph.corners[2][1]) <= y + height);
    }
}
