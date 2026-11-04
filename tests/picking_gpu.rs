//! Presented-object picking on Metal/lavapipe; no visible windows or GPU skips.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Square,
    retained::{BoardDriver, InputInbox, RetainedBoard, capture_frame, viewer_options},
    rules::ChessState,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use worldrunner::scene::{
    gpu::SceneView,
    viewer::{
        MouseButton, ObjectId, RetainedDriver, RetainedEvent, RetainedEventHost, RetainedHost,
        RetainedInput, RetainedTick, WindowlessRetained,
    },
};

type Observations = Rc<RefCell<(Option<Square>, ChessState)>>;
struct Probe {
    driver: BoardDriver,
    observations: Observations,
    event_inputs: InputInbox,
    stale: Rc<Cell<bool>>,
}
impl Probe {
    fn observe(&self) {
        *self.observations.borrow_mut() = (
            self.driver.selected(),
            self.driver.board().unwrap().game().clone(),
        );
    }
}
impl RetainedDriver for Probe {
    fn update(&mut self, host: &mut RetainedHost, tick: &RetainedTick) -> Result<(), String> {
        if self.stale.replace(false) {
            self.driver.play_uci("e2e4")?;
            self.driver.update(host, tick)?;
            let view = host.view().unwrap().unwrap();
            let point = project(&view, [0.5, 2.5, 0.05]);
            let hit = host.pick(point, |_| true).unwrap();
            assert_ne!(hit.generation, host.accepted_generation());
            click(&self.driver.inputs(), point);
        }
        self.driver.update(host, tick)?;
        self.observe();
        Ok(())
    }
    fn event(
        &mut self,
        host: &mut RetainedEventHost<'_>,
        event: &RetainedEvent,
    ) -> Result<(), String> {
        self.driver.event(host, event)?;
        if matches!(
            event,
            RetainedEvent::Presented { .. } | RetainedEvent::Capture { .. }
        ) {
            for input in self.event_inputs.borrow_mut().drain(..) {
                self.driver.event(host, &RetainedEvent::Input(input))?;
            }
        }
        self.observe();
        Ok(())
    }
}
fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').unwrap()
}
fn project(view: &SceneView, point: [f32; 3]) -> [f64; 2] {
    let screen = view
        .resolved_view()
        .world_to_screen(glam::Vec3::from_array(point))
        .unwrap();
    [f64::from(screen.x), f64::from(screen.y)]
}
fn click(inputs: &InputInbox, point: [f64; 2]) {
    inputs.borrow_mut().extend([
        RetainedInput::PointerMoved(point),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ]);
}
fn piece_id(board: &RetainedBoard, name: &str, eye: bool) -> ObjectId {
    let piece = board
        .position()
        .pieces
        .into_iter()
        .find(|p| p.square == square(name))
        .unwrap();
    let ids = board.identities()[&piece.id];
    if eye { ids.eye } else { ids.body }
}
fn fixture(preset: CameraPreset) -> (Probe, RetainedHost) {
    let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let host = RetainedHost::new(
        board.scene().clone(),
        viewer_options(board.scene(), preset, [800, 600]),
    )
    .unwrap();
    (
        Probe {
            driver: BoardDriver::with_board(board),
            observations: Rc::new(RefCell::new((None, ChessState::initial()))),
            event_inputs: InputInbox::default(),
            stale: Rc::new(Cell::new(false)),
        },
        host,
    )
}
fn capture(
    run: &mut WindowlessRetained<'_>,
    queue: &chess3d::retained::TransactionQueue,
    inbox: &chess3d::retained::CaptureInbox,
) {
    capture_frame(run, queue, inbox).unwrap();
}

#[test]
fn required_gpu_hero_tops_and_knight_eye_select_their_pieces() {
    let (mut probe, host) = fixture(CameraPreset::Hero);
    let expected = [
        (
            "e1",
            [0.5, -3.5, 1.6],
            piece_id(probe.driver.board().unwrap(), "e1", false),
        ),
        (
            "d1",
            [-0.5, -3.5, 1.44],
            piece_id(probe.driver.board().unwrap(), "d1", false),
        ),
        (
            "g1",
            [2.554, -3.582, 1.37],
            piece_id(probe.driver.board().unwrap(), "g1", true),
        ),
    ];
    let observations = probe.observations.clone();
    let events = probe.event_inputs.clone();
    let queue = probe.driver.queue();
    let inbox = probe.driver.captures();
    let mut run = host
        .windowless(&mut probe)
        .expect("GPU required, never skip");
    capture(&mut run, &queue, &inbox);
    for (name, world, id) in expected {
        let view = run.host().view().unwrap().unwrap();
        let point = project(&view, world);
        let hit = run.host().pick(point, |id| id.0 == 0 || id.0 >= 4).unwrap();
        assert_eq!(hit.hit.object, id, "fixture must hit {name} geometry");
        assert_eq!(hit.generation, run.host().accepted_generation());
        click(&events, point);
        capture(&mut run, &queue, &inbox);
        assert_eq!(observations.borrow().0, Some(square(name)));
    }
    run.shutdown().unwrap();
}

#[test]
fn required_gpu_overhead_marker_move_offboard_deselect_and_unpresented_noop() {
    let (mut probe, host) = fixture(CameraPreset::Overhead);
    let inputs = probe.driver.inputs();
    let observations = probe.observations.clone();
    let stale = probe.stale.clone();
    let queue = probe.driver.queue();
    let inbox = probe.driver.captures();
    let mut run = host
        .windowless(&mut probe)
        .expect("GPU required, never skip");
    capture(&mut run, &queue, &inbox);
    let view = run.host().view().unwrap().unwrap();
    click(&inputs, project(&view, [0.5, -2.5, 0.05]));
    capture(&mut run, &queue, &inbox);
    assert_eq!(observations.borrow().0, Some(square("e2")));
    click(&inputs, [0., 0.]);
    capture(&mut run, &queue, &inbox);
    assert_eq!(observations.borrow().0, None);
    stale.set(true);
    capture(&mut run, &queue, &inbox);
    assert_eq!(observations.borrow().0, None);
    assert_eq!(
        observations.borrow().1.piece_at(square("e4")),
        Some((chess3d::Side::Ivory, chess3d::PieceKind::Pawn))
    );
    click(&inputs, project(&view, [0.5, 2.5, 0.05]));
    capture(&mut run, &queue, &inbox);
    assert_eq!(observations.borrow().0, Some(square("e7")));
    let point = project(&view, [0.715, 0.5, 0.065]);
    let hit = run.host().pick(point, |_| true).unwrap();
    let object = run
        .host()
        .objects()
        .find(|(id, _)| *id == hit.hit.object)
        .unwrap()
        .1;
    assert_eq!(
        object.mesh().raw(),
        11,
        "fixture must hit the target ring, not its hole"
    );
    click(&inputs, point);
    capture(&mut run, &queue, &inbox);
    let mut expected = ChessState::initial();
    for uci in ["e2e4", "e7e5"] {
        expected.play(expected.parse_uci(uci).unwrap()).unwrap();
    }
    assert_eq!(observations.borrow().1, expected);
    assert_eq!(observations.borrow().0, None);
    run.shutdown().unwrap();
}
