//! Pointer button for the embedded globe (overview World tab): route to the
//! picker's own button handler (drag rotates, click focuses a cell) and report
//! whether this was a CLICK on the already-focused cell — the caller then enters
//! that world ("click a selected cell again to enter", instead of pressing
//! Enter).

use smithay::backend::input::{ButtonState, InputBackend, PointerButtonEvent};
use compositor_orchestration_core_state_base::Loop;
use compositor_y5_picker_system_base::base::{PICKER_MUT, PICKER_WORLD};

/// Press→release within this many px counts as a click, not a drag (mirrors the
/// picker's own threshold).
const CLICK_PX: f64 = 6.0;

pub fn embed_button<I: InputBackend>(
    event: &<I as InputBackend>::PointerButtonEvent,
    state: &mut Loop,
) -> bool {
    if event.state() == ButtonState::Pressed {
        compositor_y5_picker_seat_pointer::pointer::button::<I>(event, state);
        return false;
    }
    // Release: capture whether it was a click + the focused cell BEFORE the
    // picker re-selects, then route the release (which performs the select).
    let (prev, was_click) = match state
        .inner
        .worlds
        .get_mut(PICKER_WORLD)
        .storage_mut()
        .get_mut(&PICKER_MUT)
        .active
        .as_ref()
    {
        Some(a) => {
            let click = a
                .drag
                .map(|(sx, sy)| (a.pointer.0 - sx).hypot(a.pointer.1 - sy) < CLICK_PX)
                .unwrap_or(false);
            (a.selected, click)
        }
        None => (None, false),
    };
    compositor_y5_picker_seat_pointer::pointer::button::<I>(event, state);
    if !was_click {
        return false;
    }
    let now = state
        .inner
        .worlds
        .get_mut(PICKER_WORLD)
        .storage_mut()
        .get_mut(&PICKER_MUT)
        .active
        .as_ref()
        .and_then(|a| a.selected);
    now.is_some() && now == prev
}
