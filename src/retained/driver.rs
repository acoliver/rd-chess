use super::{interaction::Interaction, markers::Markers, ui::GameUi};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    sync::Arc,
};
use worldrunner::scene::{
    gpu::{CameraPose, SceneView, simulation::UiOverlay},
    viewer::{
        OrbitCamera, RetainedCaptureOutcome, RetainedDriver, RetainedEvent, RetainedEventHost,
        RetainedHost, RetainedInput, RetainedTick, RetainedUpdate, SceneObjectEdit, TimerHandle,
        ViewerError, ViewerOptions,
    },
};

pub type TransactionQueue = Rc<RefCell<VecDeque<Vec<SceneObjectEdit>>>>;
pub type CaptureInbox = Rc<RefCell<BTreeMap<u64, RetainedCaptureOutcome>>>;
pub type InputInbox = Rc<RefCell<VecDeque<RetainedInput>>>;

#[derive(Default)]
pub struct BoardDriver {
    pub(super) board: Option<super::RetainedBoard>,
    pub(super) training: Option<super::training::Training>,
    pub(super) menus: super::menus::Menus,
    pub(super) pgn: super::pgn_menus::PgnMenu,
    pub(super) label_edit: Option<super::save_labels::LabelEdit>,
    pub(super) computer: Option<super::computer::Computer>,
    pub(super) cancelled_timers: Vec<TimerHandle>,
    pub(super) storage: Option<super::persistence::Storage>,
    pub(super) review: Option<super::review_screen::ReviewSession>,
    pub(super) preferences: super::persistence::GameSettings,
    pub(super) resources: Option<RetainedUpdate>,
    pub(super) pending: TransactionQueue,
    captures: CaptureInbox,
    inputs: InputInbox,
    pub(super) interaction: Interaction,
    pub(super) markers: Markers,
    pub(super) ui: Option<GameUi>,
    overlay: Option<Arc<UiOverlay>>,
    pub(super) camera: Option<CameraControls>,
    pub(super) view: Option<SceneView>,
    pub(super) viewport: [u32; 2],
    pub(super) generation: u64,
    presented: Option<u64>,
    pub(super) presenting: bool,
    pub(super) unfocused: bool,
    pub(super) exit: bool,
}
pub(super) struct CameraControls {
    pub orbit: OrbitCamera,
    pub dirty: bool,
    pub(super) base: CameraPose,
    pub(super) limits: worldrunner::scene::viewer::OrbitLimits,
}

impl BoardDriver {
    pub fn with_board(board: super::RetainedBoard) -> Self {
        Self {
            board: Some(board),
            ..Self::default()
        }
    }
    pub fn board(&self) -> Option<&super::RetainedBoard> {
        self.board.as_ref()
    }
    pub fn configure(&mut self, options: &ViewerOptions) -> Result<(), String> {
        let side = self.board_orientation();
        let mut camera = CameraControls::new(options, side)?;
        camera.dirty = side != crate::Side::Ivory;
        self.camera = Some(camera);
        self.viewport = options.size;
        if let Some(board) = &self.board {
            let mut training = super::training::Training::bundled()?;
            if let Some(storage) = &self.storage {
                training.progress = storage.progress.clone();
            }
            let mut ui = GameUi::new().map_err(|e| e.to_string())?;
            ui.set_course_links(&training.course);
            self.training = Some(training);
            self.ui = Some(ui);
            self.markers.base = board
                .identities()
                .values()
                .map(|ids| ids.body.0.max(ids.eye.0))
                .max()
                .expect("game has kings")
                + 1;
            self.markers.base = self.markers.base.max(68);
        }
        self.refresh_game_over()?;
        self.sync_menu_ui()
    }
    pub fn camera_pose(&self) -> Option<CameraPose> {
        self.camera.as_ref().map(|camera| camera.orbit.pose())
    }
    pub fn play_uci(&mut self, uci: &str) -> Result<(), String> {
        let board = self
            .board
            .as_mut()
            .ok_or("editor scenes have no legal game")?;
        if self.menus.result.is_some()
            || !matches!(
                board.game().status(),
                crate::rules::GameStatus::Ongoing { .. }
            )
        {
            return Err("game has ended".into());
        }
        let edits = board
            .play_uci(uci)
            .map_err(|error| format!("invalid UCI move '{uci}': {error}"))?;
        self.enqueue_move(edits)
    }
    pub fn undo(&mut self) -> Result<(), String> {
        let count = self.undo_plies()?;
        let edits = self
            .board
            .as_mut()
            .expect("playable board")
            .undo_plies(count)
            .map_err(|error| error.to_string())?;
        self.menus.result = None;
        self.menus.screen = super::Screen::Play;
        self.enqueue_move(edits)
    }
    pub fn restart(&mut self) -> Result<(), String> {
        let edits = self
            .board
            .as_mut()
            .ok_or("editor scenes have no legal game")?
            .restart();
        self.menus.result = None;
        self.menus.screen = super::Screen::Play;
        self.enqueue_move(edits)
    }
    pub fn queue(&self) -> TransactionQueue {
        self.pending.clone()
    }
    pub fn captures(&self) -> CaptureInbox {
        self.captures.clone()
    }
    /// Test inputs pass through the same handler as native retained events.
    pub fn inputs(&self) -> InputInbox {
        self.inputs.clone()
    }

    fn publish_objects(&mut self, host: &mut RetainedHost) -> Result<bool, String> {
        if let Some(resources) = &self.resources {
            if admit(host, resources.clone())? {
                self.resources = None;
                self.presenting = true;
            }
            return Ok(false);
        }
        let mut pending = self.pending.borrow_mut();
        if let Some(edits) = pending.front() {
            if admit(host, RetainedUpdate::Objects(edits.clone()))? {
                pending.pop_front();
                self.presenting = true;
            }
            return Ok(false);
        }
        Ok(true)
    }
    fn publish_markers(&mut self, host: &mut RetainedHost) -> Result<bool, String> {
        let mut next = if self.preferences.markers {
            self.destinations()
        } else {
            std::collections::BTreeSet::new()
        };
        if self.preferences.markers {
            next.extend(self.selected());
        }
        let edits = self.markers.edits(&next);
        if !edits.is_empty() {
            if admit(host, RetainedUpdate::Objects(edits))? {
                self.markers.visible = next;
                self.presenting = true;
            }
            return Ok(false);
        }
        Ok(true)
    }
    fn publish_ui(&mut self, host: &mut RetainedHost) -> Result<bool, String> {
        self.sync_menu_ui()?;
        let text = self.status_text();
        let annotations = self.board_annotations();
        let view = SceneView::new(
            self.camera_pose().expect("configured camera"),
            host.options().camera,
            worldrunner::scene::gpu::Viewport {
                width: self.viewport[0],
                height: self.viewport[1],
            },
        )
        .map_err(|e| e.to_string())?;
        if let Some(ui) = &mut self.ui {
            ui.set_annotations(&annotations, &view)?;
            if let Some(overlay) = ui.overlay(&text, self.viewport)? {
                self.overlay = Some(overlay);
            }
            if let Some(overlay) = &self.overlay {
                if admit(host, RetainedUpdate::Ui(overlay.clone()))? {
                    self.overlay = None;
                    self.presenting = true;
                }
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn publish_camera(&mut self, host: &mut RetainedHost) -> Result<(), String> {
        if let Some(camera) = &mut self.camera
            && camera.dirty
            && admit(
                host,
                RetainedUpdate::Camera {
                    pose: camera.orbit.pose(),
                    lens: host.options().camera,
                    convention: host.options().camera_convention,
                },
            )?
        {
            camera.dirty = false;
            self.presenting = true;
        }
        Ok(())
    }
    fn publish(&mut self, host: &mut RetainedHost) -> Result<(), String> {
        if !self.publish_objects(host)? {
            return Ok(());
        }
        if self.viewport.contains(&0) {
            return Ok(());
        }
        if !self.publish_markers(host)? {
            return Ok(());
        }
        if !self.publish_ui(host)? {
            return Ok(());
        }
        self.publish_camera(host)
    }
}
impl RetainedDriver for BoardDriver {
    fn update(&mut self, host: &mut RetainedHost, tick: &RetainedTick) -> Result<(), String> {
        self.flush_computer_timers(|handle| {
            host.cancel_timer(handle);
        });
        if self.camera.is_none() {
            self.configure(host.options())?;
        }
        self.viewport = host.size();
        self.view = host.view().map_err(|e| e.to_string())?;
        self.generation = host.accepted_generation();
        self.presenting = self.presented != Some(self.generation);
        let inputs = std::mem::take(&mut *self.inputs.borrow_mut());
        for input in inputs {
            let hit = self.input_hit(&input, |point, include| host.pick(point, include));
            let command = host.clipboard_command(&input);
            self.input_with_clipboard(&input, hit, command, host.clipboard())?;
            if let Some(update) = self.semantic_update()? {
                host.set_accessibility(update);
            }
        }
        self.flush_clipboard(host.clipboard())?;
        host.set_text_input_area(self.text_input_area()?)
            .map_err(|e| e.to_string())?;
        self.advance_training(tick.elapsed)?;
        if self.exit {
            host.request_exit();
        } else {
            self.advance_computer(host);
            self.advance_review(host)?;
        }
        self.flush_computer_timers(|handle| {
            host.cancel_timer(handle);
        });
        if let Some(update) = self.semantic_update()? {
            host.set_accessibility(update);
        }
        let result = self.publish(host);
        self.generation = host.accepted_generation();
        result
    }

    fn event(
        &mut self,
        host: &mut RetainedEventHost<'_>,
        event: &RetainedEvent,
    ) -> Result<(), String> {
        match event {
            RetainedEvent::Accessibility(request) => self.accessibility_input(*request)?,
            RetainedEvent::AudioFinished { .. } | RetainedEvent::AudioStatusChanged { .. } => {}
            RetainedEvent::WorkReady => {
                self.drain_computer()?;
                self.drain_review();
            }
            RetainedEvent::Presented { .. } => {
                let generation = host
                    .statistics()
                    .expect("presented frame statistics")
                    .generation;
                self.presented = Some(generation);
                self.presenting = generation != self.generation;
            }
            RetainedEvent::Capture {
                id,
                generation,
                outcome,
                ..
            } => {
                if matches!(outcome, RetainedCaptureOutcome::Completed { .. }) {
                    self.presented = Some(*generation);
                    self.presenting = *generation != self.generation;
                }
                self.captures.borrow_mut().insert(*id, outcome.clone());
            }
            RetainedEvent::Input(input) => {
                let hit = self.input_hit(input, |point, include| host.pick(point, include));
                let command = host.clipboard_command(input);
                self.input_with_clipboard(input, hit, command, host.clipboard())?;
                host.set_text_input_area(self.text_input_area()?)
                    .map_err(|e| e.to_string())?;
                if self.exit {
                    host.request_exit();
                }
            }
            RetainedEvent::Timer { handle } => self.computer_timer(handle),
            RetainedEvent::Shutdown => self.computer_changed(),
        }
        self.flush_computer_timers(|handle| {
            host.cancel_timer(handle);
        });
        if let Some(update) = self.semantic_update()? {
            host.set_accessibility(update);
        }
        Ok(())
    }
}
fn admit(host: &mut RetainedHost, update: RetainedUpdate) -> Result<bool, String> {
    match host.apply(update) {
        Ok(()) => Ok(true),
        Err(ViewerError::BackPressure { .. }) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}
