//! Overview keyboard handling. The canvas router calls `handle` first; returning
//! true INTERCEPTs (see canvas `input.rs`). While open the overview consumes ALL
//! keys: Super+Tab toggles, Super+Left/Right cycle tabs, the World globe takes
//! arrows/Enter, Escape closes, every other key routes to the overview's own
//! screen-space iced surfaces (menu bar / settings) — never a client window.

use smithay::backend::input::KeyState;
use smithay::input::keyboard::{Keysym, ModifiersState};
use compositor_monitor_compositor_iced_base::IcedSpace;
use compositor_orchestration_core_state_base::Loop;
use compositor_support_library_input_keyboard_base::keyboard::key::Key;

pub fn handle(
    key: Option<Key>,
    keysym: Keysym,
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
        // Escape dismisses the logout popup first (before closing the overlay).
        if key == Some(Key::Escape) && state.inner.overview().logout.is_some() {
            compositor_y5_overview_interface_base::base::toggle_logout(state);
            return true;
        }
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
            // World tab → drive the embedded globe (no text fields here).
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
            return true;
        }
        if key == Some(Key::Escape) {
            compositor_y5_overview_interface_base::base::request_close(state);
            return true;
        }
    }
    // Non-world tabs (e.g. Settings): delegate to the focused screen-space iced
    // surface (text fields). Runs on press AND release; the overlay owns the key.
    route_screen_iced(state, keysym, key_state);
    true
}

/// Forward a key to the focused iced surface IFF it lives in screen space (the
/// overview's menu bar / settings). Modifiers update the registry; others dispatch.
fn route_screen_iced(state: &mut Loop, keysym: Keysym, key_state: KeyState) {
    let Some(reg) = state.inner.surface_mut().registry.as_mut() else { return };
    let Some(focus) = reg.keyboard_focus() else { return };
    if reg.space_of(focus) != Some(IcedSpace::Screen) {
        return;
    }
    let raw = keysym.raw();
    let pressed = matches!(key_state, KeyState::Pressed);
    if let Some(m) = compositor_monitor_compositor_iced_base::input::keysym_to_iced_modifier(raw) {
        reg.modifier_changed(m, pressed);
        return;
    }
    let eff = reg.effective_modifiers();
    let utf8 = keysym.key_char().map(|c| c.to_string());
    if let Some(e) = compositor_monitor_compositor_iced_base::registry::translate_keyboard(
        raw, utf8.as_deref(), key_state, eff, false,
    ) {
        let _ = reg.dispatch_event(focus, e);
    }
}
