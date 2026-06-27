//! Frame planning (Law 5): Status -> ordered list of passes with tap points.
//!
//! This crate owns the knowledge that used to be duplicated as a
//! `(render_scene, render_lock)` match in both backends. Backends execute the
//! plan; they do not derive it.

use compositor_kernel_graphic_draw_plan_tap::tap::{TapPoint, POST_SCENE};
use compositor_orchestration_core_state_base::state::Status;

/// A render pass kind. The *meaning* of each kind (which element source feeds
/// it) is compositor vocabulary; backends only map kinds to element sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePass {
    /// The desktop scene (windows, layers).
    Scene,
    /// The session-lock scene, composited above whatever preceded it.
    Lock,
    /// The world-selection screen (an overlay world). Mutually exclusive with
    /// Scene/Lock — when the picker is the active world it owns the frame.
    Picker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStep {
    Pass(FramePass),
    Tap(TapPoint),
}

#[derive(Debug, Clone, Default)]
pub struct FramePlan {
    pub steps: Vec<PlanStep>,
}

impl FramePlan {
    pub fn is_empty(&self) -> bool {
        !self.steps.iter().any(|s| matches!(s, PlanStep::Pass(_)))
    }

    pub fn has_pass(&self, pass: FramePass) -> bool {
        self.steps.iter().any(|s| matches!(s, PlanStep::Pass(p) if *p == pass))
    }

    pub fn has_tap(&self, tap: TapPoint) -> bool {
        self.steps.iter().any(|s| matches!(s, PlanStep::Tap(t) if *t == tap))
    }
}

/// Encode exactly today's pass ordering (behavior-preserving, Phase 0):
/// - Running / Unlock          -> Scene, Tap(post-scene)
/// - Locked { pending: true }  -> Scene, Tap(post-scene), Lock
/// - Locked { pending: false } -> Lock
/// - Sleep / Terminate         -> (empty)
///
/// `picker_active` is whether the world-selection overlay is the active world.
/// It overrides everything below: the picker owns the whole frame (it is only
/// ever reached from `Status::Running`, and the origin world is suspended). The
/// caller passes it (rather than this crate knowing the PICKER world id) so the
/// plan crate stays free of expansion dependencies.
///
/// The picker pass still carries the post-scene tap: an in-flight screen/
/// full-screen capture keeps recording while the user navigates worlds, so the
/// intermediate world-picker overlay shows up in the video instead of the frame
/// freezing on the last desktop frame. (Window/world-region captures are
/// screen-space-independent and simply keep tracking their windows.)
pub fn plan(status: &Status, picker_active: bool) -> FramePlan {
    use PlanStep::*;
    if picker_active {
        return FramePlan { steps: vec![Pass(FramePass::Picker), Tap(POST_SCENE)] };
    }
    let steps = match status {
        Status::Running | Status::Unlock { .. } => {
            vec![Pass(FramePass::Scene), Tap(POST_SCENE)]
        }
        Status::Locked { pending, .. } => {
            if *pending {
                vec![Pass(FramePass::Scene), Tap(POST_SCENE), Pass(FramePass::Lock)]
            } else {
                vec![Pass(FramePass::Lock)]
            }
        }
        Status::Sleep { .. } | Status::Terminate => vec![],
    };
    FramePlan { steps }
}
