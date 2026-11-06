use crate::{
    CameraPreset,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use worldrunner::{
    scene::viewer::{KeyCode, LogicalKey, RetainedInput},
    ui::UiNodeId,
};

#[test]
fn keyboard_focus_uses_the_native_render_plan_and_semantic_focus() {
    let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
        .handle_input(
            &RetainedInput::Key {
                key: "arrowdown".into(),
                logical: LogicalKey::Character("arrowdown".into()),
                physical: KeyCode::ArrowDown.into(),
                pressed: true,
                repeat: false,
            },
            None,
            None,
        )
        .unwrap();
    let update = driver.semantic_update().unwrap().unwrap();
    let node = update
        .changed
        .iter()
        .find(|node| node.id == update.focus)
        .unwrap();
    assert_eq!(node.label, "a7, black pawn");
    let UiNodeId::Object(focus) = update.focus else {
        panic!("focused board control");
    };
    let ui = driver.ui.as_mut().unwrap();
    ui.overlay("White to move", [800, 600]).unwrap();
    assert_eq!(ui.native.router.focused(), Some(focus));
    let plan = ui.plan.as_ref().unwrap();
    let graphic = plan
        .graphics
        .iter()
        .find(|graphic| graphic.graphic.object == focus)
        .unwrap();
    assert_eq!(plan.graphics.len(), 1);
    assert_eq!(graphic.graphic.quads.len(), 4);
    assert!(
        graphic
            .graphic
            .quads
            .iter()
            .all(|quad| quad.color.map(f32::to_bits) == [1., 0.72, 0.20, 1.].map(f32::to_bits))
    );
}
