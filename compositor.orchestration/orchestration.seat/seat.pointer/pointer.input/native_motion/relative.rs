use crate::native_motion::dispatch;
use smithay::backend::input::{Axis, AxisSource, Event, InputBackend, PointerAxisEvent};
use smithay::input::pointer::{AxisFrame, MotionEvent, PointerHandle};
use smithay::utils::{Logical, Physical, Point, SERIAL_COUNTER};
use compositor_y5_camera_transform_translate::translate;
use compositor_orchestration_core_state_base::Loop;
use compositor_y5_surface_interface_base::hit;
use compositor_y5_surface_interface_base::hit::SurfaceHit;

pub fn input_received_normalized<I: InputBackend>(
    event: &I::PointerMotionEvent,
    _loop: &mut Loop,
    position_normalized: Point<f64, Logical>,
    position_screen: &Point<f64, Logical>,
    delta: (Point<f64, Logical>, Point<f64, Logical>),
    was_constrain_locked: bool,
) {
    // Overview World tab → the overview layer drives the globe (drag-rotate),
    // unless the cursor is over the menu bar (it returns false → fall through for
    // bar hover).
    if compositor_y5_overview_input_pointer::pointer::relative::<I>(event, _loop) {
        return;
    }

    let position_normalized = position_normalized.clone().into();

    let serial = SERIAL_COUNTER.next_serial();
    let pointer = _loop.state.seat.seat.get_pointer().unwrap();

    dispatch::dispatch(
        _loop,
        event.time_msec(),
        serial,
        pointer,
        position_normalized,
        Some(delta),
        was_constrain_locked,
    );
}
