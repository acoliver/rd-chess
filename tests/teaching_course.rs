use chess3d::teaching::{Course, Goal, LessonRunner, Progress, Step, TaskRunner, TaskStatus};
use serde_json::{Value, json};

fn document() -> Value {
    json!({"version":1,"lessons":[{"id":"test","title":"Try a move","requires":[],
        "steps":[{"kind":"task","text":"Move the pawn to e4.",
        "fen":chess3d::rules::INITIAL_FEN,"side":"ivory","goal":"line",
        "solutions":[{"uci":"e2e4","next":[]}],
        "hints":["Look at the e-pawn.","Play e2-e4."],
        "feedback":{"d2d4":"Use the e-pawn for this exercise."},"annotations":{}}]}]})
}

fn load(value: &Value) -> Result<Course, chess3d::teaching::TeachingError> {
    Course::from_json(&value.to_string())
}

fn reject(change: impl FnOnce(&mut Value), message: &str) {
    let mut value = document();
    change(&mut value);
    let error = load(&value).unwrap_err().to_string();
    assert!(error.contains(message), "{error}");
}

#[test]
fn schema_rejects_bad_version_duplicate_and_missing_ids() {
    reject(|v| v["version"] = json!(2), "version");
    reject(
        |v| {
            v["lessons"]
                .as_array_mut()
                .unwrap()
                .push(document()["lessons"][0].clone());
        },
        "duplicate",
    );
    reject(
        |v| v["lessons"][0]["requires"] = json!(["absent"]),
        "prerequisite",
    );
}

#[test]
fn schema_rejects_cycles_and_illegal_positions() {
    reject(|v| v["lessons"][0]["requires"] = json!(["test"]), "cycle");
    reject(
        |v| v["lessons"][0]["steps"][0]["fen"] = json!("8/8/8/8/8/8/8/8 w - - 0 1"),
        "FEN",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["side"] = json!("onyx"),
        "side",
    );
}

#[test]
fn schema_rejects_illegal_unreachable_and_false_mates() {
    reject(
        |v| v["lessons"][0]["steps"][0]["solutions"][0]["uci"] = json!("e2e5"),
        "move",
    );
    reject(
        |v| {
            let branch = v["lessons"][0]["steps"][0]["solutions"][0].clone();
            v["lessons"][0]["steps"][0]["solutions"]
                .as_array_mut()
                .unwrap()
                .push(branch);
        },
        "unreachable",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["goal"] = json!("checkmate"),
        "mate",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["hints"] = json!([""]),
        "text",
    );
}

#[test]
fn schema_rejects_unknown_fields_and_oversize_text() {
    reject(|v| v["surprise"] = json!(true), "unknown field");
    reject(
        |v| v["lessons"][0]["steps"][0]["hints"] = json!(["x".repeat(501)]),
        "text",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["annotations"] = json!({"squares":["i9"]}),
        "square",
    );
}

fn exercise_paths(
    task: &chess3d::teaching::Task,
    prefix: &[String],
    nodes: &[chess3d::teaching::MoveNode],
) {
    for node in nodes {
        let mut path = prefix.to_vec();
        path.push(node.uci.clone());
        if node.next.is_empty() {
            let mut runner = TaskRunner::new(task.clone()).unwrap();
            for uci in path.iter().step_by(2) {
                runner.submit(uci).unwrap();
            }
            assert_eq!(runner.status(), &TaskStatus::Completed);
        } else {
            exercise_paths(task, &path, &node.next);
        }
    }
}

#[test]
fn every_shipped_lesson_and_branch_completes() {
    let course = Course::bundled().unwrap();
    assert_eq!(course.lessons().len(), 12);
    for lesson in course.lessons() {
        for step in &lesson.steps {
            if let Step::Task(task) = step {
                exercise_paths(task, &[], &task.solutions);
            }
        }
    }
}

#[test]
fn wrong_feedback_hints_retry_and_progress_round_trip() {
    let course = load(&document()).unwrap();
    let lesson = course.lessons()[0].clone();
    let mut runner = LessonRunner::new(lesson).unwrap();
    runner.submit("d2d4").unwrap();
    assert_eq!(
        runner.task().unwrap().status(),
        &TaskStatus::Wrong("Use the e-pawn for this exercise.".into())
    );
    assert_eq!(runner.hint(), Some("Look at the e-pawn."));
    assert_eq!(runner.hint(), Some("Play e2-e4."));
    assert_eq!(runner.hint(), None);
    runner.retry().unwrap();
    assert_eq!(runner.task().unwrap().hint_level(), 0);
    runner.submit("e2e4").unwrap();
    runner.advance().unwrap();
    assert!(runner.completed());
    let progress = runner.progress();
    assert!(progress.completed_lessons.contains("test"));
    assert_eq!(progress.lessons["test"].attempts, 2);
    assert_eq!(progress.lessons["test"].hints_used, 2);
    assert_eq!(
        Progress::from_json(&progress.to_json().unwrap()).unwrap(),
        *progress
    );
}

#[test]
fn any_mating_move_is_accepted_but_line_tasks_remain_specific() {
    let course = Course::bundled().unwrap();
    let task = course
        .lessons()
        .iter()
        .flat_map(|l| &l.steps)
        .find_map(|s| match s {
            Step::Task(t) if t.goal == Goal::AnyMate => Some(t),
            _ => None,
        })
        .unwrap();
    let state = chess3d::rules::ChessState::from_fen(&task.fen).unwrap();
    let mut count = 0;
    for chess_move in state.legal_moves() {
        let mut after = state.clone();
        after.play(chess_move).unwrap();
        if matches!(after.status(), chess3d::rules::GameStatus::Checkmate { .. }) {
            let mut runner = TaskRunner::new(task.clone()).unwrap();
            runner.submit(&chess_move.to_uci()).unwrap();
            assert_eq!(runner.status(), &TaskStatus::Completed);
            if !task
                .solutions
                .iter()
                .any(|node| node.uci == chess_move.to_uci())
            {
                let mut specific = task.clone();
                specific.goal = Goal::Checkmate;
                let mut runner = TaskRunner::new(specific).unwrap();
                runner.submit(&chess_move.to_uci()).unwrap();
                assert!(matches!(runner.status(), TaskStatus::Wrong(_)));
            }
            count += 1;
        }
    }
    assert!(count > 1, "exercise must contain alternative mates");
}

#[test]
fn unreachable_opponent_branches_and_opponent_leaves_are_rejected() {
    reject(
        |v| {
            v["lessons"][0]["steps"][0]["solutions"][0]["next"] = json!([
                {"uci":"e7e5","next":[{"uci":"g1f3","next":[]}]},
                {"uci":"c7c5","next":[{"uci":"g1f3","next":[]}]}
            ]);
        },
        "opponent branch",
    );
    reject(
        |v| {
            v["lessons"][0]["steps"][0]["solutions"][0]["next"] = json!([
                {"uci":"e7e5","next":[]}
            ]);
        },
        "opponent turn",
    );
    reject(
        |v| {
            v["lessons"][0]["steps"][0]["solutions"][0]["next"] = json!([
                {"uci":"e7e6","next":[{"uci":"e4e6","next":[]}]}
            ]);
        },
        "move e4e6",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["solutions"] = json!([]),
        "empty solution",
    );
}

#[test]
fn feedback_is_bounded_and_unicode_move_keys_report_errors() {
    reject(
        |v| v["lessons"][0]["steps"][0]["feedback"] = json!({"d2d4":""}),
        "text",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["feedback"] = json!({"d2d4":"x".repeat(501)}),
        "text",
    );
    reject(
        |v| v["lessons"][0]["steps"][0]["feedback"] = json!({"aé1":"No."}),
        "ASCII UCI",
    );
}

#[test]
fn multi_lesson_cycles_and_empty_titles_are_rejected() {
    reject(
        |v| {
            let mut other = v["lessons"][0].clone();
            other["id"] = json!("other");
            other["requires"] = json!(["test"]);
            v["lessons"][0]["requires"] = json!(["other"]);
            v["lessons"].as_array_mut().unwrap().push(other);
        },
        "cycle",
    );
    reject(|v| v["lessons"][0]["title"] = json!(" "), "text");
}

#[test]
fn runner_guards_steps_and_wrong_moves_leave_board_unchanged() {
    let course = Course::bundled().unwrap();
    let mut lesson = LessonRunner::new(course.lessons()[0].clone()).unwrap();
    assert!(lesson.submit("e2e4").is_err());
    assert_eq!(lesson.hint(), None);
    lesson.advance().unwrap();
    assert!(lesson.advance().is_err());
    let before = lesson.task().unwrap().board().to_fen();
    lesson.submit("e2e5").unwrap();
    assert_eq!(lesson.task().unwrap().board().to_fen(), before);
    assert!(matches!(
        lesson.task().unwrap().status(),
        TaskStatus::Wrong(_)
    ));
    lesson.submit("d2d4").unwrap();
    assert!(
        matches!(lesson.task().unwrap().status(), TaskStatus::Wrong(text) if text.contains("d-pawn"))
    );
    assert_eq!(lesson.task().unwrap().board().to_fen(), before);
    lesson.submit("e2e4").unwrap();
    assert!(lesson.submit("e7e5").is_err());
    assert_eq!(lesson.hint(), None);
}

#[test]
fn saved_totals_survive_new_sessions_and_bad_progress_is_rejected() {
    let course = load(&document()).unwrap();
    let mut runner = LessonRunner::new(course.lessons()[0].clone()).unwrap();
    runner.submit("d2d4").unwrap();
    runner.hint();
    let progress = Progress::from_json(&runner.progress().to_json().unwrap()).unwrap();
    let mut restored = LessonRunner::with_progress(course.lessons()[0].clone(), progress).unwrap();
    restored.submit("e2e4").unwrap();
    restored.advance().unwrap();
    assert_eq!(restored.progress().lessons["test"].attempts, 2);
    assert_eq!(restored.progress().lessons["test"].hints_used, 1);
    assert!(restored.advance().is_err());
    assert!(restored.retry().is_err());
    let mut value = serde_json::to_value(restored.progress()).unwrap();
    value["version"] = json!(2);
    assert!(Progress::from_json(&value.to_string()).is_err());
    value["version"] = json!(1);
    value["bands"] = json!({"under-1200":{"attempts":0,"solved":1}});
    assert!(Progress::from_json(&value.to_string()).is_err());
}
