use crate::native_press;
use smithay::backend::input::{ButtonState, InputBackend, PointerButtonEvent};
use compositor_orchestration_core_state_base::Loop;
use compositor_y5_surface_interface_base::hit::surface_under_filtered;
use compositor_y5_window_interface_draw::visible::DrawWindow;

pub fn button<I: InputBackend>(event: &<I as InputBackend>::PointerButtonEvent, _loop: &mut Loop) {
    // Overview overlay open → the overview layer handles + swallows the click
    // (menu bar / grid cell / globe); windows never receive it.
    if compositor_y5_overview_input_pointer::pointer::button::<I>(event, _loop) {
        return;
    }

    let pointer = &_loop.state.seat.seat.get_pointer().unwrap();
    {
        // World input bus first (phase 3); Pass falls through to legacy routing.
        let location = pointer.current_location();
        let ev = compositor_support_system_input_event_base::base::InputEvent::PointerButton {
            button: event.button_code(),
            pressed: event.state() == ButtonState::Pressed,
            x: location.x,
            y: location.y,
        };
        if compositor_orchestration_input_drive_base::drive::route(_loop, ev)
            == compositor_support_system_input_event_base::base::InputFlow::Consume
        {
            return;
        }
    }
    let event = &event;

    let button_state = event.state();
    let keyboard = &_loop.state.seat.seat.get_keyboard().unwrap();

    // PRESS and RELEASE are both handled by `CanvasSystem::input` on the world bus
    // (route() above). A `Pass` from the bus on PRESS means the cursor is over a
    // window (the system cleared selection + declined to grab), so the click is
    // routed directly to that window here via `native_press`.
    if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
        // Overview overlay open → presentational: never deliver a click to a
        // window (the bus already routes menu-bar iced clicks).
        let overview_open = _loop.inner.overview().visible;
        if let Some(hit) = surface_under_filtered(_loop, pointer.current_location(), &|hit| {
            if let Some(window) = hit.window() {
                return !overview_open && window.visible(_loop);
            };

            true
        }) {
            // It is directly over a window.
            native_press::press::input_received::<I>(
                pointer, event, _loop, hit, keyboard, button_state,
            )
        }
    }
}
