use compositor_support_system_storage_token_base::base::{Token, TokenMut};

/// World-storage tokens for the camera slot (beside the type — no
/// system-crate dependency needed to read camera state).
pub static CAMERA: Token<Camera> = Token::new();
/// TRANSITIONAL pub: legacy input/draw paths still write directly.
pub static CAMERA_MUT: TokenMut<Camera> = TokenMut::new(&CAMERA);

use std::collections::HashMap;
use compositor_y5_camera_transform_state::state::Transform;
use compositor_y5_camera_zone_state::state::CameraZone;

#[derive(Default)]
pub struct Camera {
    pub transform: compositor_y5_camera_transform_state::state::Transform,
    pub zone: compositor_y5_camera_zone_state::state::CameraZone,
    pub position_previous: smithay::utils::Point<f64, smithay::utils::Logical>,
    /// Momentum-pan state (touchpad two-finger swipe). `pan_accum` collects the
    /// world-space pan delta within a frame; the camera system converts it to a
    /// per-second `pan_velocity` each tick, then — once the swipe ends — coasts
    /// the camera along that velocity with exponential friction. `panning` is true
    /// while the swipe is live (velocity is measured, not yet coasting);
    /// `pan_idle_frames` counts frames with no pan delta to detect the lift-off.
    pub pan_velocity: smithay::utils::Point<f64, smithay::utils::Logical>,
    pub pan_accum: smithay::utils::Point<f64, smithay::utils::Logical>,
    pub panning: bool,
    pub pan_idle_frames: u32,
    /// Set when the touchpad reports lift-off (terminating 0,0 finger axis), so the
    /// coast launches on the NEXT tick with no idle delay — the snappy release.
    pub pan_ending: bool,
}

