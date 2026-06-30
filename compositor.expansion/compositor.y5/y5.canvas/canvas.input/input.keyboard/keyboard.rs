use crate::{input, navigator};
use smithay::backend::input::KeyState;
use smithay::input::keyboard::{Keysym, ModifiersState, keysyms};
use compositor_orchestration_core_state_base::Loop;
use compositor_support_library_input_keyboard_base::keyboard::key::Key;

pub fn input_received(
    key_state: KeyState,
    modifiers: &ModifiersState,
    keysym: Keysym, // D = Dispatch flip: the caller extracts the keysym from the
                    // smithay seat callback (which only sees `&mut Dispatch`) so
                    // this runs against the full `&mut Loop`.
    state: &mut Loop,
) -> Option<bool> {
    let key = Key::from_keysym(keysym);

    // Overview (Super+Tab) owns its keys: toggling from any state, and fully
    // capturing the keyboard while open. When it consumes the key we INTERCEPT
    // (None) — the overview has already delegated to its own screen-space iced
    // surfaces, and the key must reach neither the canvas tools, the focused
    // window, nor the fallback iced stage.
    if compositor_y5_overview_input_keyboard::keyboard::handle(key, keysym, key_state, modifiers, state) {
        return None;
    }

    input::input_received(key_state, modifiers, state, key)?;
    navigator::input_received(key_state, modifiers, state, key)?;

    Some(true)
}
