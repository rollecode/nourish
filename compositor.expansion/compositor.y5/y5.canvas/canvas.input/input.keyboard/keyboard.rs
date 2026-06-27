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
    // capturing the keyboard while open. The overview layer decides; if it
    // consumed the key, windows get nothing.
    if compositor_y5_overview_input_keyboard::keyboard::handle(key, key_state, modifiers, state) {
        return Some(true);
    }

    input::input_received(key_state, modifiers, state, key)?;
    navigator::input_received(key_state, modifiers, state, key)?;

    Some(true)
}
