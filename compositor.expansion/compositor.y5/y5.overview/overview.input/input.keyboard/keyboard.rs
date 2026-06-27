//! Overview keyboard handling, encapsulated. The canvas keyboard router calls
//! `handle` first; it returns true if the overview consumed the key (Super+Tab
//! toggles from anywhere; while open, all keys are swallowed and Super+Left/Right
//! cycle tabs, the World tab's arrows/Enter drive the globe, Escape closes).

use smithay::backend::input::KeyState;
use smithay::input::keyboard::ModifiersState;
use compositor_orchestration_core_state_base::Loop;
use compositor_support_library_input_keyboard_base::keyboard::key::Key;

pub fn handle(
    key: Option<Key>,
    key_state: KeyState,
    modifiers: &ModifiersState,
    state: &mut Loop,
) -> bool {
    let press = key_state == KeyState::Pressed;
    // Super (logo), or Ctrl when running nested.
    let modkey = if state.inner.storage.nested { modifiers.ctrl } else { modifiers.logo };

    // Super+Tab toggles the overlay from any state.
    if press && modkey && key == Some(Key::Tab) {
        compositor_y5_overview_interface_base::base::toggle(state);
        return true;
    }
    if !state.inner.overview().visible {
        return false;
    }
    if press {
        // Super+Left/Right cycle the tabs in any tab.
        if modkey && key == Some(Key::Left) {
            compositor_y5_overview_interface_surface::surface::cycle_tab(state, false);
            return true;
        }
        if modkey && key == Some(Key::Right) {
            compositor_y5_overview_interface_surface::surface::cycle_tab(state, true);
            return true;
        }
        if state.inner.overview().is_world() {
            // World tab → drive the embedded globe.
            match key {
                Some(Key::Left) => compositor_y5_picker_interface_embed::embed::select_direction(state, -1, 0),
                Some(Key::Right) => compositor_y5_picker_interface_embed::embed::select_direction(state, 1, 0),
                Some(Key::Up) => compositor_y5_picker_interface_embed::embed::select_direction(state, 0, 1),
                Some(Key::Down) => compositor_y5_picker_interface_embed::embed::select_direction(state, 0, -1),
                Some(Key::Return) => compositor_y5_overview_interface_activate::activate::activate_world(state),
                Some(Key::Escape) => {
                    compositor_y5_overview_interface_base::base::request_close(state);
                }
                _ => {}
            }
        } else if key == Some(Key::Escape) {
            compositor_y5_overview_interface_base::base::request_close(state);
        }
    }
    // While open, the overlay owns the keyboard — windows receive nothing.
    true
}
