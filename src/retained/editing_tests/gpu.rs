use super::*;
use crate::retained::{InputInbox, capture_frame};
use worldrunner::scene::viewer::RetainedHost;

fn queue_key(inputs: &InputInbox, key: &str, pressed: bool) {
    inputs.borrow_mut().push_back(RetainedInput::Key {
        key: key.into(),
        logical: LogicalKey::Character(key.into()),
        physical: KeyCode::Enter.into(),
        pressed,
        repeat: false,
    });
}
fn press(inputs: &InputInbox, key: &str) {
    for pressed in [true, false] {
        queue_key(inputs, key, pressed);
    }
}

#[test]
#[ignore = "requires the final windowless Metal suite"]
fn native_label_field_draws_and_commits_through_the_host_input_queue() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.save_game(1).unwrap();
    slots(&mut driver);
    let scene = driver.board().unwrap().scene().clone();
    let host = RetainedHost::new(
        scene.clone(),
        super::super::viewer_options(&scene, CameraPreset::Overhead, [800, 600]),
    )
    .unwrap();
    let inputs = driver.inputs();
    let queue = driver.queue();
    let captures = driver.captures();
    let mut run = host.windowless(&mut driver).expect("Metal required");
    let before = capture_frame(&mut run, &queue, &captures).unwrap();
    inputs.borrow_mut().extend([
        RetainedInput::PointerMoved([400., 204.]),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ]);
    let field = capture_frame(&mut run, &queue, &captures).unwrap();
    assert_ne!(field.pixels, before.pixels);
    press(&inputs, "home");
    queue_key(&inputs, "shift", true);
    press(&inputs, "end");
    queue_key(&inputs, "shift", false);
    inputs
        .borrow_mut()
        .push_back(RetainedInput::Text(TextInput::Commit(
            "Metal opening".into(),
        )));
    let edited = capture_frame(&mut run, &queue, &captures).unwrap();
    assert_ne!(edited.pixels, field.pixels);
    assert_eq!(
        edited.pixels,
        capture_frame(&mut run, &queue, &captures).unwrap().pixels
    );
    press(&inputs, "enter");
    capture_frame(&mut run, &queue, &captures).unwrap();
    run.shutdown().unwrap();
    drop(run);
    assert_eq!(driver.screen(), Screen::Slots);
    assert_eq!(driver.slot_labels()[0], "Metal opening");
}
