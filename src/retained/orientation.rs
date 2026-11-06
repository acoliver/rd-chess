use super::{BoardDriver, driver::CameraControls};
use crate::Side;
use worldrunner::scene::viewer::{OrbitCamera, ViewerOptions};

impl CameraControls {
    pub(super) fn new(options: &ViewerOptions, side: Side) -> Result<Self, String> {
        let mut controls = Self {
            orbit: OrbitCamera::new(options.pose, options.orbit).map_err(|e| e.to_string())?,
            dirty: true,
            base: options.pose,
            limits: options.orbit,
        };
        controls.orient(side)?;
        Ok(controls)
    }
    fn orient(&mut self, side: Side) -> Result<(), String> {
        let mut pose = self.base;
        if side == Side::Onyx {
            for axis in 0..2 {
                pose.eye_millimeters[axis] = -pose.eye_millimeters[axis];
                pose.target_millimeters[axis] = -pose.target_millimeters[axis];
            }
        }
        self.orbit = OrbitCamera::new(pose, self.limits).map_err(|e| e.to_string())?;
        self.dirty = true;
        Ok(())
    }
}
impl BoardDriver {
    pub(super) fn board_orientation(&self) -> Side {
        self.computer_game()
            .map_or(Side::Ivory, super::ComputerGame::human)
    }
    pub(super) fn orient_board(&mut self, side: Side) -> Result<(), String> {
        if let Some(camera) = &mut self.camera {
            camera.orient(side)?;
            self.view = None;
        }
        Ok(())
    }
}
