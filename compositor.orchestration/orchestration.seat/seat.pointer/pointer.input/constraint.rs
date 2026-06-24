use smithay::{
    input::pointer::PointerHandle,
    utils::{Logical, Point},
    wayland::{
        pointer_constraints::{PointerConstraint, PointerConstraintRef, with_pointer_constraint},
        seat::WaylandFocus,
    },
};
use compositor_orchestration_core_state_base::Loop;

pub fn apply_pointer_constraint(
    _loop: &mut Loop,
    previous_pointer_location: Point<f64, Logical>,
    candidate: Point<f64, Logical>,
) -> (Point<f64, Logical>, bool) {
    let Some(pointer) = _loop.state.seat.seat.get_pointer() else {
        return (candidate, false);
    };

    let Some(focused) = pointer.current_focus() else {
        return (candidate, false);
    };
    let Some(focused_surface) = focused.wl_surface() else {
        return (candidate, false);
    };

    let surface_origin = _loop
        .inner.space_state()
        .element_location_for_surface(&focused_surface)
        .to_f64();

    // Resolve the surface size HERE, BEFORE `with_pointer_constraint` takes the
    // surface's per-surface data lock. `element_size_for_surface` internally does
    // `Window::geometry` -> `with_states`, which locks that same surface; calling it
    // inside the `with_pointer_constraint` closure (which already holds the lock)
    // re-enters the non-reentrant mutex and deadlocks the main thread (hard freeze on
    // any pointer-locked/confined client drag, e.g. Blender). Precompute, like
    // `surface_origin` above.
    let surface_size = _loop
        .inner.space_state()
        .element_size_for_surface(&focused_surface);

    let mut result = candidate;
    let mut active = false;

    with_pointer_constraint(&focused_surface, &pointer, |constraint| {
        let Some(constraint) = constraint else { return };
        if !constraint.is_active() { return }
        active = true;

        match &*constraint {
            PointerConstraint::Locked(_) => {
                result = previous_pointer_location;
            }
            PointerConstraint::Confined(confined) => {
                let candidate_local = candidate - surface_origin;
                let inside = match confined.region() {
                    Some(region) => region.contains((
                        candidate_local.x.floor() as i32,
                        candidate_local.y.floor() as i32,
                    )),
                    None => {
                        let size = surface_size;
                        candidate_local.x >= 0.0
                            && candidate_local.y >= 0.0
                            && candidate_local.x < size.w as f64
                            && candidate_local.y < size.h as f64
                    }
                };
                result = if inside { candidate } else { previous_pointer_location };
            }
        }
    });

    (result, active)
}