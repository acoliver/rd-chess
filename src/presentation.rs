pub fn shadow_settings() -> worldrunner::scene::gpu::shadows::DirectionalShadowSettings {
    worldrunner::scene::gpu::shadows::DirectionalShadowSettings {
        focus_meters: glam::Vec3::ZERO,
        half_extent_meters: 8.0,
        depth_half_extent_meters: 16.0,
        resolution: 2048,
        depth_bias: 0.0008,
        normal_bias_meters: 0.012,
        refresh_policy: worldrunner::scene::gpu::shadows::CascadeRefreshPolicy::default(),
    }
}

use worldrunner::scene::gpu::CameraPose;

/// Game-owned camera presets shared by capture, window viewing and acceptance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraPreset {
    Hero,
    Overhead,
    Lineup,
}

impl CameraPreset {
    pub fn pose(self) -> CameraPose {
        match self {
            Self::Hero => CameraPose {
                eye_millimeters: [8_200, -10_500, 10_000],
                target_millimeters: [0, 0, 400],
            },
            Self::Overhead => CameraPose {
                // Stay clear of the Z-up orbit pole in both capture and window mode.
                eye_millimeters: [0, -1_500, 13_000],
                target_millimeters: [0, 0, 0],
            },
            Self::Lineup => CameraPose {
                eye_millimeters: [0, -5_000, 3_200],
                target_millimeters: [0, 0, 350],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CameraPreset;

    #[test]
    fn overhead_pose_is_inside_window_orbit_limits() {
        let pose = CameraPreset::Overhead.pose();
        let offset = glam::DVec3::from_array(pose.eye_millimeters.map(|v| v as f64))
            - glam::DVec3::from_array(pose.target_millimeters.map(|v| v as f64));
        let elevation = (offset.z / offset.length()).asin();
        assert!(elevation > 0.1 && elevation < 1.47);
        assert!(offset.length() > 2_000.0 && offset.length() < 40_000.0);
        assert_eq!(pose.target_millimeters, [0, 0, 0]);
    }
}
